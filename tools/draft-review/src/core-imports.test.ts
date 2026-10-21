import {describe, expect, test} from 'bun:test'
import {readdirSync, readFileSync} from 'node:fs'
import {join} from 'node:path'
import ts from 'typescript'

const CORE_FILES = ['args', 'contribution', 'contribution-set', 'draft', 'exit', 'ledger', 'lists', 'parsed', 'payload', 'render', 'synthesize', 'uploads']
const ALLOWED_PACKAGES = new Set(['zod', 'node:crypto', 'node:util'])
const FORBIDDEN_GLOBALS = [/\bBun\b/, /\bprocess\b/, /\brequire\s*\(/, /\bDate\.now\b/, /\bnew\s+(globalThis\.)?Date\b/, /\bglobalThis\s*\[/]

const sourceOf = (name: string) => readFileSync(join(import.meta.dir, `${name}.ts`), 'utf8')
const importsOf = (source: string) => [...source.matchAll(/(?:import|export)\s[^'"]*?from\s+['"]([^'"]+)['"]|import\s*\(\s*['"]([^'"]+)['"]\s*\)|import\s+['"]([^'"]+)['"]/g)].map(match => match[1] ?? match[2] ?? match[3] ?? '')
const isSiblingCore = (specifier: string) => specifier.startsWith('./') && CORE_FILES.includes(specifier.slice(2).replace(/\.ts$/, ''))

const LITERAL_KINDS = new Set([
  ts.SyntaxKind.StringLiteral,
  ts.SyntaxKind.NoSubstitutionTemplateLiteral,
  ts.SyntaxKind.TemplateHead,
  ts.SyntaxKind.TemplateMiddle,
  ts.SyntaxKind.TemplateTail,
  ts.SyntaxKind.RegularExpressionLiteral
])

type Range = {pos: number; end: number}

const tokensOf = (node: ts.Node, file: ts.SourceFile): ts.Node[] => {
  const children = node.getChildren(file)
  return children.length === 0 ? [node] : children.flatMap(child => tokensOf(child, file))
}

const commentsBefore = (text: string, token: ts.Node): Range[] => [
  ...(ts.getLeadingCommentRanges(text, token.getFullStart()) ?? []),
  ...(ts.getTrailingCommentRanges(text, token.getFullStart()) ?? [])
]

// A parser, not a regular expression, finds the literals: a regex literal can hold a quote that would derail a hand-written scanner.
function codeOnly(source: string): string {
  const file = ts.createSourceFile('core.ts', source, ts.ScriptTarget.Latest, true)
  const tokens = tokensOf(file, file)
  const blanked: Range[] = [
    ...tokens.flatMap(token => commentsBefore(source, token)),
    ...tokens.filter(token => LITERAL_KINDS.has(token.kind)).map(token => ({pos: token.getStart(file), end: token.getEnd()}))
  ]
  const hidden = new Set(blanked.flatMap(range => Array.from({length: range.end - range.pos}, (_slot, offset) => range.pos + offset)))
  return source
    .split('')
    .map((unit, index) => (hidden.has(index) && unit !== '\n' ? ' ' : unit))
    .join('')
}

const forbiddenNames = (source: string) => {
  const code = codeOnly(source)
  return FORBIDDEN_GLOBALS.filter(pattern => pattern.test(code)).map(String)
}

describe('the forbidden-name check', () => {
  test('ignores the forbidden words inside comments and string literals', () => {
    const source = [
      '// The shell reads process.env, not the core.',
      '/* Bun.spawn lives in github.ts. */',
      "const note = 'process the findings'",
      'const other = "Bun runs the shell"',
      'const template = `require(${note}) and Date.now()`',
      'const pattern = /\\bprocess\\b/'
    ].join('\n')
    expect(forbiddenNames(source)).toEqual([])
  })

  test('finds a forbidden name inside a template expression', () => {
    expect(forbiddenNames('const text = `pid ${process.pid}`')).toEqual([String(/\bprocess\b/)])
  })

  test('finds Date.now and new Date in code, with or without parentheses', () => {
    expect(forbiddenNames('const stamp = Date.now()')).toEqual([String(/\bDate\.now\b/)])
    expect(forbiddenNames('const today = new Date()')).toEqual([String(/\bnew\s+(globalThis\.)?Date\b/)])
    expect(forbiddenNames('const today = new Date;')).toEqual([String(/\bnew\s+(globalThis\.)?Date\b/)])
    expect(forbiddenNames('const today = new globalThis.Date()')).toEqual([String(/\bnew\s+(globalThis\.)?Date\b/)])
  })

  test('finds a global that code reaches through globalThis with a string key', () => {
    expect(forbiddenNames("const env = globalThis['process'].env")).toEqual([String(/\bglobalThis\s*\[/)])
  })
})

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

  test.each(CORE_FILES)('%s does not name Bun, process, require, Date.now, or new Date', name => {
    expect(forbiddenNames(sourceOf(name))).toEqual([])
  })
})
