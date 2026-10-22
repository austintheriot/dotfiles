import {describe, expect, test} from 'bun:test'
import {planAdd} from './add-plan'
import type {Contribution} from './contribution'
import {devQa, review, staleToken, trace, verify, windowsDevQa} from './testing/fixtures'

const stored = (entries: [string, Contribution][]) => entries.map(([name, contribution]) => ({name, contribution}))
const listingOf = (entries: [string, Contribution][]) => entries.flatMap(([name]) => [name, `${name}.files`])
const plan = (contribution: Contribution, entries: [string, Contribution][]) => planAdd({contribution, listing: listingOf(entries), stored: stored(entries)})

const reproducedR1 = verify([{id: 'R1', result: 'reproduced', evidence: ['/tmp/qa/r1.mp4'], note: 'Seen.'}])

describe('planAdd', () => {
  test('stores a first contribution under its file name and removes nothing', () => {
    expect(plan(review(), [])).toEqual({kind: 'store', name: 'code-review.json', remove: []})
  })

  test('a new contribution replaces the stored file of the same name, so the old one does not count against the merge rules', () => {
    expect(plan(review({body: 'Second.'}), [['code-review.json', review()]])).toEqual({kind: 'store', name: 'code-review.json', remove: []})
  })

  test('refuses a dev-qa-verify verdict that answers no review finding with a behavior, and names the rule', () => {
    const unrouted = verify([{id: 'R2', result: 'reproduced', evidence: ['/tmp/qa/r2.mp4'], note: 'Seen.'}])
    expect(plan(unrouted, [['code-review.json', review()]])).toEqual({
      kind: 'refuse',
      reason: 'a dev-qa-verify verdict answers R2, which is not a review finding with a behavior'
    })
  })

  test('refuses a second Dev QA part that has no platform beside one that has a platform', () => {
    const result = plan(devQa(), [['dev-qa@Windows.json', windowsDevQa()]])
    expect(result).toEqual({kind: 'refuse', reason: '2 Dev QA contributions need distinct platforms, and 1 of them have no platform'})
  })

  test('refuses a round-two contribution when no round-one contribution is stored', () => {
    expect(plan(reproducedR1, [])).toEqual({kind: 'refuse', reason: 'no code-review or dev-qa contribution to merge'})
  })

  test('checks a new code-review without the dev-qa-verify files that it removes', () => {
    const withoutBehavior = review({findings: [{id: staleToken.id, kind: 'code', title: staleToken.title, body: staleToken.body}]})
    const entries: [string, Contribution][] = [['code-review.json', review()], ['dev-qa-verify.json', reproducedR1]]
    expect(plan(withoutBehavior, entries)).toEqual({kind: 'store', name: 'code-review.json', remove: ['dev-qa-verify.json', 'dev-qa-verify.json.files']})
  })

  test('stores a new code-review that drops the review finding that a stored code-trace names in duplicateOf, and removes the code-trace', () => {
    const duplicate = trace([{id: 'Q1', result: 'root-cause', body: 'Same cause.', duplicateOf: 'R1'}])
    const entries: [string, Contribution][] = [['code-review.json', review()], ['dev-qa.json', devQa()], ['code-trace.json', duplicate]]
    expect(plan(review({findings: [], comments: []}), entries)).toEqual({
      kind: 'store',
      name: 'code-review.json',
      remove: ['code-trace.json', 'code-trace.json.files']
    })
  })
})
