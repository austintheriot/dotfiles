import {describe, expect, test} from 'bun:test'
import {DraftReviewError, EXIT} from './exit'
import {isPendingReviewConflict, parseHttpReply, recordedReviewFrom} from './github'

const reply = (statusLine: string, body: string) => `${statusLine}\r\nContent-Type: application/json\r\nX-Github-Request-Id: 1\r\n\r\n${body}`

describe('parseHttpReply', () => {
  test('splits the status code from the body', () => {
    expect(parseHttpReply(reply('HTTP/2.0 200 OK', '{"id":1}'))).toEqual({status: 200, body: '{"id":1}'})
  })

  test('gives no status for text that is not an HTTP reply', () => {
    expect(parseHttpReply('gh: connection refused')).toEqual({status: undefined, body: 'gh: connection refused'})
  })
})

describe('recordedReviewFrom', () => {
  const record = {reviewId: 7, bodyHash: 'a'.repeat(64), comments: [], head: 'a'.repeat(40), url: 'https://example.test/review/7'}
  const run = (code: number, stdout: string, stderr = '') => ({code, stdout, stderr})

  test('reads a 404 reply as a deleted review', () => {
    expect(recordedReviewFrom(run(1, reply('HTTP/2.0 404 Not Found', '{"message":"Not Found"}'), 'gh: Not Found (HTTP 404)'), record)).toEqual({kind: 'gone', record})
  })

  test('reads HTTP 404 on stderr as a deleted review when stdout has no status line', () => {
    expect(recordedReviewFrom(run(1, '', 'gh: Not Found (HTTP 404)'), record)).toEqual({kind: 'gone', record})
  })

  test('reads the state of a review that exists', () => {
    expect(recordedReviewFrom(run(0, reply('HTTP/2.0 200 OK', '{"id":7,"state":"COMMENTED"}')), record)).toEqual({kind: 'found', record, state: 'COMMENTED'})
  })

  test('stops with exit 69 on any other failure', () => {
    const caught = (() => {
      try {
        return recordedReviewFrom(run(1, reply('HTTP/2.0 500 Internal Server Error', '{}'), 'gh: HTTP 500'), record)
      } catch (error) {
        return error instanceof DraftReviewError ? error.exitCode : String(error)
      }
    })()
    expect(caught).toBe(EXIT.unavailable)
  })
})

describe('isPendingReviewConflict', () => {
  const conflict = '{"message":"Unprocessable Entity","errors":["User can only have one pending review per pull request"],"status":"422"}'

  test('is true for a 422 that says the user already has a pending review', () => {
    expect(isPendingReviewConflict(parseHttpReply(reply('HTTP/2.0 422 Unprocessable Entity', conflict)))).toBe(true)
  })

  test('is false for a 422 with another message', () => {
    expect(isPendingReviewConflict(parseHttpReply(reply('HTTP/2.0 422 Unprocessable Entity', '{"message":"Validation Failed","errors":["line must be part of the diff"]}')))).toBe(false)
  })

  test('is false for the same message under another status', () => {
    expect(isPendingReviewConflict(parseHttpReply(reply('HTTP/2.0 500 Internal Server Error', conflict)))).toBe(false)
  })
})
