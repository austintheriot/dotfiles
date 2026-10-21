import {describe, expect, test} from 'bun:test'
import type {ReviewPayload} from './synthesize'
import {citedPaths, substituteUploads} from './uploads'

const payload = (body: string, commentBody = body): ReviewPayload => ({
  body,
  comments: [{path: 'src/a.ts', line: 1, side: 'RIGHT', body: commentBody}],
  uploads: ['/a.png', '/a.png.mp4'],
  handback: ['Unchanged.']
})

describe('substituteUploads', () => {
  test('replaces each whole path in the body and in every comment', () => {
    const urls = new Map([['/a.png', 'https://cdn.example/1'], ['/a.png.mp4', 'https://cdn.example/2']])
    expect(substituteUploads(payload('See /a.png and /a.png.mp4.'), urls)).toEqual({
      body: 'See https://cdn.example/1 and https://cdn.example/2.',
      comments: [{path: 'src/a.ts', line: 1, side: 'RIGHT', body: 'See https://cdn.example/1 and https://cdn.example/2.'}],
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

describe('citedPaths', () => {
  test('finds whole paths in order of first citation, once each', () => {
    expect(citedPaths(['/a.png.mp4 then /b.png', 'and /b.png again'], ['/b.png', '/a.png', '/a.png.mp4'])).toEqual(['/a.png.mp4', '/b.png'])
  })

  test('finds nothing when there are no candidates', () => {
    expect(citedPaths(['/a.png'], [])).toEqual([])
  })
})
