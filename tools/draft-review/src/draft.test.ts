import {describe, expect, test} from 'bun:test'
import {bodyHash, contributionsDigest, draftRecordAfterCreate} from './draft'
import type {ReadBack} from './draft'
import {HEAD} from './testing/fixtures'

const DIGEST = 'd'.repeat(64)
const created = {id: 7, body: 'Body.', url: 'https://example.test/review'}
const comment = (id: number, body: string) => ({id, body})
const afterCreate = (requestedBodies: string[], readBack: ReadBack) =>
  draftRecordAfterCreate({review: created, head: HEAD, requestedBodies, readBack, contributionsDigest: DIGEST})

describe('draftRecordAfterCreate', () => {
  test('records the review, each read-back comment id with its body hash, and the contributions digest', () => {
    expect(afterCreate(['First.', 'Second.'], {kind: 'read', comments: [comment(12, 'Second.'), comment(11, 'First.')]})).toEqual({
      kind: 'record',
      record: {
        reviewId: 7,
        bodyHash: bodyHash('Body.'),
        comments: [
          {id: 12, bodyHash: bodyHash('Second.')},
          {id: 11, bodyHash: bodyHash('First.')}
        ],
        contributionsDigest: DIGEST,
        head: HEAD,
        url: 'https://example.test/review'
      }
    })
  })

  test('records a review with two comments of the same body', () => {
    expect(afterCreate(['Same.', 'Same.'], {kind: 'read', comments: [comment(11, 'Same.'), comment(12, 'Same.')]}).kind).toBe('record')
  })

  test('skips the record when GitHub shows more comments than the request had, because one can be the user', () => {
    const decision = afterCreate(['First.'], {kind: 'read', comments: [comment(11, 'First.'), comment(12, 'Mine.')]})
    expect(decision).toEqual({kind: 'skip', cause: 'mismatch', reason: 'GitHub shows 2 inline comments on pending review 7, and the request had 1'})
  })

  test('skips the record when GitHub shows fewer comments than the request had', () => {
    expect(afterCreate(['First.', 'Second.'], {kind: 'read', comments: [comment(11, 'First.')]})).toMatchObject({kind: 'skip', cause: 'mismatch'})
  })

  test('skips the record when the count matches but the bodies differ', () => {
    const decision = afterCreate(['First.', 'First.'], {kind: 'read', comments: [comment(11, 'First.'), comment(12, 'Mine.')]})
    expect(decision).toEqual({kind: 'skip', cause: 'mismatch', reason: 'the inline comments on pending review 7 are not the comments that the request sent'})
  })

  test('records when a read-back body differs from the request only in CRLF line ends', () => {
    expect(afterCreate(['Line one.\n\n- item'], {kind: 'read', comments: [comment(11, 'Line one.\r\n\r\n- item')]}).kind).toBe('record')
  })

  test('records when a read-back body differs from the request only in a trailing newline', () => {
    expect(afterCreate(['Body text.\n'], {kind: 'read', comments: [comment(11, 'Body text.')]}).kind).toBe('record')
  })

  test('skips the record when a read-back body differs from the request in its text, not only in line ends', () => {
    expect(afterCreate(['Line one.\n- item'], {kind: 'read', comments: [comment(11, 'Line one.\r\n- other')]})).toMatchObject({kind: 'skip', cause: 'mismatch'})
  })

  test('skips the record when the comments could not be read back, and names the failure', () => {
    expect(afterCreate(['First.'], {kind: 'failed', reason: 'gh could not list the comments: HTTP 502'})).toEqual({
      kind: 'skip',
      cause: 'read-failed',
      reason: 'gh could not list the comments: HTTP 502'
    })
  })
})

describe('contributionsDigest', () => {
  const files = [
    {name: 'code-review.json', sha256: 'a'.repeat(64)},
    {name: 'dev-qa.json', sha256: 'b'.repeat(64)}
  ]

  test('does not depend on the order of the listing', () => {
    expect(contributionsDigest([...files].reverse())).toBe(contributionsDigest(files))
  })

  test('changes when a file content changes', () => {
    const changed = files.map(file => (file.name === 'dev-qa.json' ? {...file, sha256: 'c'.repeat(64)} : file))
    expect(contributionsDigest(changed)).not.toBe(contributionsDigest(files))
  })

  test('changes when a file is added, removed, or renamed', () => {
    const digest = contributionsDigest(files)
    expect(contributionsDigest([...files, {name: 'code-trace.json', sha256: 'c'.repeat(64)}])).not.toBe(digest)
    expect(contributionsDigest(files.slice(0, 1))).not.toBe(digest)
    expect(contributionsDigest(files.map(file => ({...file, name: `x${file.name}`})))).not.toBe(digest)
  })
})
