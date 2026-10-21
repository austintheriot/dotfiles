import {describe, expect, test} from 'bun:test'
import {anchorProblems, parsePatch} from './diff-anchors'
import type {InlineComment} from './payload'

const TWO_HUNKS = [
  '@@ -10,6 +10,8 @@ export function refresh() {',
  '   const token = read()',
  '-  if (expired(token)) {',
  '+  if (expired(token, now)) {',
  '+    log()',
  '+    renew()',
  '   }',
  '   return token',
  '@@ -50,3 +52,2 @@ function later() {',
  '   one()',
  '-  two()',
  '   three()'
].join('\n')

const comment = (overrides: Partial<InlineComment> = {}): InlineComment => ({path: 'src/a.ts', line: 12, side: 'RIGHT', body: 'Text.', source: 'code-trace', ...overrides})

describe('parsePatch', () => {
  test('gives each hunk its line range on each side', () => {
    expect(parsePatch(TWO_HUNKS)).toEqual({
      right: [
        {start: 10, end: 17},
        {start: 52, end: 53}
      ],
      left: [
        {start: 10, end: 15},
        {start: 50, end: 52}
      ]
    })
  })

  test('gives a new file only right-side lines', () => {
    expect(parsePatch('@@ -0,0 +1,3 @@\n+one\n+two\n+three')).toEqual({right: [{start: 1, end: 3}], left: []})
  })

  test('reads a hunk header that omits the count as one line', () => {
    expect(parsePatch('@@ -4 +4 @@\n-old\n+new')).toEqual({right: [{start: 4, end: 4}], left: [{start: 4, end: 4}]})
  })

  test('gives a deleted-only hunk left-side lines and no right-side lines', () => {
    expect(parsePatch('@@ -5,3 +4,0 @@\n-one\n-two\n-three')).toEqual({right: [], left: [{start: 5, end: 7}]})
  })

  test('does not count a no-newline marker as a line', () => {
    expect(parsePatch('@@ -1,2 +1,2 @@\n one\n-two\n\\ No newline at end of file\n+two\n\\ No newline at end of file')).toEqual({
      right: [{start: 1, end: 2}],
      left: [{start: 1, end: 2}]
    })
  })

  test('gives no ranges for an empty patch', () => {
    expect(parsePatch('')).toEqual({right: [], left: []})
  })
})

describe('anchorProblems', () => {
  const files = [
    {filename: 'src/a.ts', patch: TWO_HUNKS},
    {filename: 'src/new.ts', patch: '@@ -0,0 +1,3 @@\n+one\n+two\n+three'},
    {filename: 'assets/logo.png', patch: undefined},
    {filename: 'src/renamed.ts', previousFilename: 'src/old-name.ts', patch: '@@ -1,2 +1,2 @@\n-a\n+b\n c'}
  ]

  test('accepts a comment on a changed line or a context line of any hunk', () => {
    expect(anchorProblems([comment({line: 12}), comment({line: 17}), comment({line: 53})], files)).toEqual([])
  })

  test('refuses a comment between two hunks, naming the file, the line, and the source contribution', () => {
    expect(anchorProblems([comment({line: 30})], files)).toEqual([
      'the inline comment on src/a.ts line 30 (RIGHT) from code-trace is outside every hunk of the pull request diff on the RIGHT side'
    ])
  })

  test('accepts any added line of a new file, and refuses a line past its end', () => {
    expect(anchorProblems([comment({path: 'src/new.ts', line: 3, source: 'dev-qa@macOS'})], files)).toEqual([])
    expect(anchorProblems([comment({path: 'src/new.ts', line: 4, source: 'dev-qa@macOS'})], files)).toEqual([
      'the inline comment on src/new.ts line 4 (RIGHT) from dev-qa@macOS is outside every hunk of the pull request diff on the RIGHT side'
    ])
  })

  test('refuses a LEFT comment on a new file, which has no left side', () => {
    expect(anchorProblems([comment({path: 'src/new.ts', line: 1, side: 'LEFT'})], files)).toHaveLength(1)
  })

  test('checks a LEFT comment against the old line numbers', () => {
    expect(anchorProblems([comment({line: 51, side: 'LEFT'})], files)).toEqual([])
    expect(anchorProblems([comment({line: 16, side: 'LEFT'})], files)).toHaveLength(1)
  })

  test('refuses a comment on a file that GitHub sends with no patch', () => {
    expect(anchorProblems([comment({path: 'assets/logo.png', line: 1, source: 'code-review'})], files)).toEqual([
      'the inline comment on assets/logo.png line 1 (RIGHT) from code-review is on a file that GitHub sends with no patch, so draft-review cannot check the line'
    ])
  })

  test('accepts the new name of a renamed file and refuses its old name', () => {
    expect(anchorProblems([comment({path: 'src/renamed.ts', line: 1})], files)).toEqual([])
    expect(anchorProblems([comment({path: 'src/old-name.ts', line: 1})], files)).toEqual([
      'the inline comment on src/old-name.ts line 1 (RIGHT) from code-trace is on a file that the pull request does not change'
    ])
  })

  test('refuses a multi-line comment whose first line and last line sit in different hunks', () => {
    expect(anchorProblems([comment({startLine: 16, line: 52})], files)).toEqual([
      'the inline comment on src/a.ts lines 16-52 (RIGHT) from code-trace is outside every hunk of the pull request diff on the RIGHT side'
    ])
  })

  test('accepts a multi-line comment inside one hunk', () => {
    expect(anchorProblems([comment({startLine: 11, line: 14})], files)).toEqual([])
  })

  test('gives one problem for each comment that fails', () => {
    expect(anchorProblems([comment({line: 30}), comment({line: 12}), comment({path: 'src/gone.ts'})], files)).toHaveLength(2)
  })
})
