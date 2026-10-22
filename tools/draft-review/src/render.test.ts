import {expect, test} from 'bun:test'
import {renderReview, renderStrayPaths} from './render'

test('renders the body, each inline comment with its location, the uploads, and the hand-back', () => {
  const text = renderReview({
    body: 'Body text.',
    comments: [
      {path: 'src/a.ts', line: 42, side: 'RIGHT', body: 'One line.', source: 'code-review'},
      {path: 'src/b.ts', line: 9, side: 'LEFT', startLine: 7, body: 'A range.', source: 'code-trace'}
    ],
    uploads: ['/tmp/qa/a.png'],
    handback: ['Check the head.']
  })
  expect(text).toBe(
    [
      'Body text.',
      '',
      '--- inline comment: src/a.ts line 42 (RIGHT) ---',
      '',
      'One line.',
      '',
      '--- inline comment: src/b.ts lines 7-9 (LEFT) ---',
      '',
      'A range.',
      '',
      '--- files to upload ---',
      '',
      '/tmp/qa/a.png',
      '',
      '--- hand-back (not posted) ---',
      '',
      'Check the head.',
      ''
    ].join('\n')
  )
})

test('marks each local path that create refuses, with its location', () => {
  expect(renderStrayPaths([{location: 'the body', path: '/Users/someone/a.png'}])).toBe(
    '--- local paths that create refuses: list each file in uploads, or remove the path ---\n\nthe body: /Users/someone/a.png\n'
  )
})

test('marks nothing when no local path is left', () => {
  expect(renderStrayPaths([])).toBe('')
})

test('renders only the body when there is nothing else', () => {
  expect(renderReview({body: 'Body.', comments: [], uploads: [], handback: []})).toBe('Body.\n')
})
