import {describe, expect, test} from 'bun:test'
import {mkdtempSync, readFileSync, rmSync} from 'node:fs'
import {tmpdir} from 'node:os'
import {join} from 'node:path'
import {DraftReviewError, EXIT} from './exit'
import {ghUploader, isPendingReviewConflict, parseHttpReply, recordedReviewFrom, reviewRequest} from './github'
import type {InlineComment, ReviewPayload} from './payload'

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

describe('reviewRequest', () => {
  const head = 'c'.repeat(40)
  const payload = (comments: ReviewPayload['comments']): ReviewPayload => ({body: 'Body.', comments, uploads: [], handback: ['Not posted.']})
  const comment = (overrides: Partial<InlineComment> = {}): InlineComment => ({path: 'src/a.ts', line: 42, side: 'RIGHT', body: 'Comment.', source: 'code-review', ...overrides})

  test('never names an event, so GitHub keeps the review pending', () => {
    expect(Object.keys(reviewRequest(head, payload([comment()])))).not.toContain('event')
    expect(Object.keys(reviewRequest(head, payload([])))).not.toContain('event')
  })

  test('sends the head, the body, and each comment, and leaves the hand-back out', () => {
    expect(reviewRequest(head, payload([comment()]))).toEqual({commit_id: head, body: 'Body.', comments: [{path: 'src/a.ts', line: 42, side: 'RIGHT', body: 'Comment.'}]})
  })

  test('maps a multi-line comment to start_line and start_side on the side of the comment', () => {
    const request = reviewRequest(head, payload([comment({startLine: 40, side: 'LEFT'})]))
    expect(request.comments).toEqual([{path: 'src/a.ts', line: 42, side: 'LEFT', start_line: 40, start_side: 'LEFT', body: 'Comment.'}])
  })

  test('sends no start_line when startLine equals line', () => {
    expect(reviewRequest(head, payload([comment({startLine: 42})])).comments).toEqual([{path: 'src/a.ts', line: 42, side: 'RIGHT', body: 'Comment.'}])
  })

  test('sends no comments key when the review has no inline comments', () => {
    expect(reviewRequest(head, payload([]))).toEqual({commit_id: head, body: 'Body.'})
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

describe('ghUploader', () => {
  test('sends the upload with a Content-Length, which the GitHub upload endpoint requires', async () => {
    const dir = mkdtempSync(join(tmpdir(), 'draft-review-upload-'))
    const log = join(dir, 'requests.jsonl')
    const serverScript = `
      const server = Bun.serve({port: 0, fetch: async request => {
        const body = await request.arrayBuffer()
        require('node:fs').appendFileSync(${JSON.stringify(log)}, JSON.stringify({length: request.headers.get('content-length'), encoding: request.headers.get('transfer-encoding'), bytes: body.byteLength}) + '\\n')
        return Response.json({url: 'https://example.com/asset.png'})
      }})
      console.log(server.port)
    `
    const server = Bun.spawn(['bun', '-e', serverScript], {stdout: 'pipe'})
    const token = process.env.GH_TOKEN
    process.env.GH_TOKEN = 'test-token'
    try {
      const reader = server.stdout.getReader()
      const port = new TextDecoder().decode((await reader.read()).value).trim()
      const bytes = new Uint8Array(5000).fill(7)
      const url = await ghUploader(`http://localhost:${port}/assets`, 1).upload({path: '/tmp/shot.png', bytes})
      expect(url).toBe('https://example.com/asset.png')
      const requests = readFileSync(log, 'utf8').trim().split('\n').map(line => JSON.parse(line))
      expect(requests).toEqual([{length: '5000', encoding: null, bytes: 5000}])
    } finally {
      if (token === undefined) delete process.env.GH_TOKEN
      else process.env.GH_TOKEN = token
      server.kill()
      rmSync(dir, {recursive: true, force: true})
    }
  })
})
