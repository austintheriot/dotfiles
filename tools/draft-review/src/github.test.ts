import {describe, expect, test} from 'bun:test'
import {isPendingReviewConflict, parseHttpReply} from './github'

const reply = (statusLine: string, body: string) => `${statusLine}\r\nContent-Type: application/json\r\nX-Github-Request-Id: 1\r\n\r\n${body}`

describe('parseHttpReply', () => {
  test('splits the status code from the body', () => {
    expect(parseHttpReply(reply('HTTP/2.0 200 OK', '{"id":1}'))).toEqual({status: 200, body: '{"id":1}'})
  })

  test('gives no status for text that is not an HTTP reply', () => {
    expect(parseHttpReply('gh: connection refused')).toEqual({status: undefined, body: 'gh: connection refused'})
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
