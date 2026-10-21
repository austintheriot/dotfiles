import {describe, expect, test} from 'bun:test'
import {readdirSync, readFileSync} from 'node:fs'
import {join} from 'node:path'

const CORE_FILES = ['args', 'contribution', 'contribution-set', 'draft', 'exit', 'ledger', 'lists', 'parsed', 'payload', 'render', 'synthesize', 'uploads']
const ALLOWED_PACKAGES = new Set(['zod', 'node:crypto', 'node:util'])
const FORBIDDEN_GLOBALS = [/\bBun\b/, /\bprocess\b/, /\brequire\s*\(/]

const sourceOf = (name: string) => readFileSync(join(import.meta.dir, `${name}.ts`), 'utf8')
const importsOf = (source: string) => [...source.matchAll(/(?:import|export)\s[^'"]*?from\s+['"]([^'"]+)['"]|import\s*\(\s*['"]([^'"]+)['"]\s*\)|import\s+['"]([^'"]+)['"]/g)].map(match => match[1] ?? match[2] ?? match[3] ?? '')
const isSiblingCore = (specifier: string) => specifier.startsWith('./') && CORE_FILES.includes(specifier.slice(2).replace(/\.ts$/, ''))

describe('the core files stay pure', () => {
  test('every core file exists, and every source file that is not core is named as shell', () => {
    const sources = readdirSync(import.meta.dir).filter(name => name.endsWith('.ts') && !name.endsWith('.test.ts')).map(name => name.replace(/\.ts$/, ''))
    const shell = ['config', 'github', 'lock', 'main', 'store']
    expect([...sources].sort()).toEqual([...CORE_FILES, ...shell].sort())
  })

  test.each(CORE_FILES)('%s imports only sibling core files, zod, node:crypto, and node:util', name => {
    const disallowed = importsOf(sourceOf(name)).filter(specifier => !isSiblingCore(specifier) && !ALLOWED_PACKAGES.has(specifier))
    expect(disallowed).toEqual([])
  })

  test.each(CORE_FILES)('%s does not name Bun, process, or require', name => {
    const source = sourceOf(name)
    expect(FORBIDDEN_GLOBALS.filter(pattern => pattern.test(source)).map(String)).toEqual([])
  })
})
