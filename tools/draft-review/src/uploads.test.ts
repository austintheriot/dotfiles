import {describe, expect, test} from 'bun:test'
import type {ReviewPayload} from './payload'
import {citedPaths, strayLocalPaths, substituteUploads} from './uploads'

const payload = (body: string, commentBody = body): ReviewPayload => ({
  body,
  comments: [{path: 'src/a.ts', line: 1, side: 'RIGHT', body: commentBody, source: 'code-review'}],
  uploads: ['/a.png', '/a.png.mp4'],
  handback: ['Unchanged.']
})

describe('substituteUploads', () => {
  test('replaces each whole path in the body and in every comment', () => {
    const urls = new Map([['/a.png', 'https://cdn.example/1'], ['/a.png.mp4', 'https://cdn.example/2']])
    expect(substituteUploads(payload('See /a.png and /a.png.mp4.'), urls)).toEqual({
      body: 'See https://cdn.example/1 and https://cdn.example/2.',
      comments: [{path: 'src/a.ts', line: 1, side: 'RIGHT', body: 'See https://cdn.example/1 and https://cdn.example/2.', source: 'code-review'}],
      uploads: ['/a.png', '/a.png.mp4'],
      handback: ['Unchanged.']
    })
  })

  test('does not replace a path inside a longer path', () => {
    const urls = new Map([['/a.png', 'https://cdn.example/1']])
    expect(substituteUploads(payload('/a.png.mp4 and /x/a.png and ![](/a.png)'), urls).body).toBe('/a.png.mp4 and /x/a.png and ![](https://cdn.example/1)')
  })

  test('returns the payload unchanged when no path has a URL', () => {
    const original = payload('See /a.png.')
    expect(substituteUploads(original, new Map())).toEqual(original)
  })
})

describe('strayLocalPaths', () => {
  const withText = (body: string, commentBody = 'Fine.', uploads: string[] = []): ReviewPayload => ({
    body,
    comments: [{path: 'src/a.ts', line: 7, side: 'RIGHT', startLine: 5, body: commentBody, source: 'code-review'}],
    uploads,
    handback: ['/Users/someone/handback-is-never-posted.txt']
  })

  test('names each absolute local path in the body and in each comment, with its location', () => {
    expect(strayLocalPaths(withText('See /Users/someone/a.png and ~/notes.md.', 'Log at /private/var/x.log'))).toEqual([
      {location: 'the body', path: '/Users/someone/a.png'},
      {location: 'the body', path: '~/notes.md'},
      {location: 'the inline comment on src/a.ts lines 5-7', path: '/private/var/x.log'}
    ])
  })

  test('finds paths under /home/, /tmp/, and /var/folders/', () => {
    expect(strayLocalPaths(withText('(/home/dev/out.txt), /tmp/run/1.mp4, and /var/folders/ab/T/shot.png')).map(stray => stray.path)).toEqual([
      '/home/dev/out.txt',
      '/tmp/run/1.mp4',
      '/var/folders/ab/T/shot.png'
    ])
  })

  test('finds a file URL with a local path', () => {
    expect(strayLocalPaths(withText('Open file:///Users/someone/report.html.')).map(stray => stray.path)).toEqual(['file:///Users/someone/report.html'])
  })

  test('skips a path in an inline code span', () => {
    expect(strayLocalPaths(withText('The cache is `/tmp/cache.json`, and ``~/a `b` /home/x`` too.'))).toEqual([])
  })

  test('skips a path in a fenced code block, with a quoted string inside it', () => {
    expect(strayLocalPaths(withText('Before.\n```sh\ncp "/home/node/app" /tmp/out\n```\nAfter.'))).toEqual([])
  })

  test('skips a path in a suggestion block of an inline comment', () => {
    expect(strayLocalPaths(withText('Clean.', 'Use this:\n```suggestion\nconst dir = "/tmp/app"\n```'))).toEqual([])
  })

  test('skips a path in an unclosed fenced block that runs to the end of the text', () => {
    expect(strayLocalPaths(withText('Before.\n~~~\n/Users/someone/x'))).toEqual([])
  })

  test('finds a path after a closed fenced block and in a quoted string outside code', () => {
    expect(strayLocalPaths(withText("```\n/tmp/inside\n```\nSee '/home/dev/out.txt'.")).map(stray => stray.path)).toEqual(['/home/dev/out.txt'])
  })

  test('ignores a path that uploads lists, because create puts its URL in place', () => {
    expect(strayLocalPaths(withText('See /tmp/qa/blank.png.', 'Fine.', ['/tmp/qa/blank.png']))).toEqual([])
  })

  test('ignores a URL whose path holds one of the prefixes', () => {
    expect(strayLocalPaths(withText('See https://cdn.example/tmp/a.png and https://example.test/Users/x.'))).toEqual([])
  })

  test('ignores the hand-back lines, which create never posts', () => {
    expect(strayLocalPaths(withText('Clean.'))).toEqual([])
  })
})

describe('citedPaths', () => {
  test('finds whole paths in order of first citation, once each', () => {
    expect(citedPaths(['/a.png.mp4 then /b.png', 'and /b.png again'], ['/b.png', '/a.png', '/a.png.mp4'])).toEqual(['/a.png.mp4', '/b.png'])
  })

  test('prefers the longest path when a shorter path is its prefix', () => {
    expect(citedPaths(['see /a b.png and /a.'], ['/a', '/a b.png'])).toEqual(['/a b.png', '/a'])
  })

  test('does not match a path that a non-ASCII letter continues', () => {
    expect(citedPaths(['/tmp/café.png and /tmp/naïve'], ['/tmp/caf', '/tmp/na'])).toEqual([])
  })

  test('matches a non-ASCII path as a whole', () => {
    expect(citedPaths(['see /tmp/café.png.'], ['/tmp/café.png'])).toEqual(['/tmp/café.png'])
  })

  test('finds nothing when there are no candidates', () => {
    expect(citedPaths(['/a.png'], [])).toEqual([])
  })
})
