import {describe, expect, test} from 'bun:test'
import {namesToClear, staleRoundTwoNames} from './clear'

describe('staleRoundTwoNames', () => {
  const listing = [
    'code-review.json',
    'code-review.json.files',
    'dev-qa@macOS.json',
    'dev-qa@macOS.json.files',
    'dev-qa-verify.json',
    'dev-qa-verify.json.files',
    'dev-qa-verify@macOS.json',
    'dev-qa-verify@macOS.json.files',
    'code-trace.json',
    'code-trace.json.files',
    'synthesized.md'
  ]

  test('a code-review removes every dev-qa-verify file and its hash record, and keeps the code-trace', () => {
    expect(staleRoundTwoNames('code-review', listing)).toEqual([
      'dev-qa-verify.json',
      'dev-qa-verify.json.files',
      'dev-qa-verify@macOS.json',
      'dev-qa-verify@macOS.json.files'
    ])
  })

  test('a dev-qa removes the code-trace file and its hash record, and keeps every dev-qa-verify', () => {
    expect(staleRoundTwoNames('dev-qa', listing)).toEqual(['code-trace.json', 'code-trace.json.files'])
  })

  test('a round-two kind removes nothing', () => {
    expect(staleRoundTwoNames('dev-qa-verify', listing)).toEqual([])
    expect(staleRoundTwoNames('code-trace', listing)).toEqual([])
  })

  test('removes a hash record whose round-two contribution is gone', () => {
    expect(staleRoundTwoNames('code-review', ['dev-qa-verify@Windows.json.files', 'notes.json'])).toEqual(['dev-qa-verify@Windows.json.files'])
  })

  test('gives no names for an empty listing', () => {
    expect(staleRoundTwoNames('dev-qa', [])).toEqual([])
  })
})

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
