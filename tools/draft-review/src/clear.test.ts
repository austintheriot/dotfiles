import {describe, expect, test} from 'bun:test'
import {namesToClear} from './clear'

describe('namesToClear', () => {
  test('selects each contribution file and its hash record', () => {
    expect(namesToClear(['code-review.json', 'code-review.json.files', 'dev-qa@macOS.json', 'dev-qa@macOS.json.files'])).toEqual([
      'code-review.json',
      'code-review.json.files',
      'dev-qa@macOS.json',
      'dev-qa@macOS.json.files'
    ])
  })

  test('keeps every file that is not a contribution or a hash record of one', () => {
    expect(namesToClear(['synthesized.md', 'payload.request', 'notes.json', 'notes.json.files', '.code-review.json.12.abc.tmp', 'code-review.json'])).toEqual([
      'code-review.json'
    ])
  })

  test('selects a hash record whose contribution is gone', () => {
    expect(namesToClear(['code-trace.json.files', 'synthesized.md'])).toEqual(['code-trace.json.files'])
  })

  test('gives no names for an empty listing', () => {
    expect(namesToClear([])).toEqual([])
  })
})
