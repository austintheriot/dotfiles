import {describe, expect, test} from 'bun:test'
import type {Finding, TraceVerdict, VerifyVerdict} from './contribution'
import {routeFindings} from './contribution-set'
import {blankPage, devQa, HEAD, OTHER_HEAD, refusalOf, review, setOf, staleToken, trace, verify, windowsDevQa} from './test-fixtures'

describe('routeFindings', () => {
  test('sends review findings with a behavior to verify and skips the others', () => {
    expect(routeFindings(setOf([review()])).toVerify).toEqual([staleToken])
  })

  test('sends Dev QA behavior findings to trace and routes code findings nowhere', () => {
    const untested: Finding = {id: 'Q2', kind: 'code', title: 'Step 3 not run', body: 'No test machine for step 3.'}
    expect(routeFindings(setOf([devQa({findings: [blankPage(), untested]})]))).toEqual({toVerify: [], toTrace: [blankPage()]})
  })

  test('qualifies Dev QA ids with their platform and keeps two platforms apart', () => {
    const set = setOf([devQa({platform: 'macOS'}), windowsDevQa()])
    expect(routeFindings(set).toTrace.map(finding => finding.id)).toEqual(['Q1@macOS', 'Q1@Windows'])
  })

  test('never qualifies review ids', () => {
    expect(routeFindings(setOf([review(), devQa({platform: 'macOS'})])).toVerify.map(finding => finding.id)).toEqual(['R1'])
  })
})

describe('validateContributions', () => {
  test('groups the contributions by kind', () => {
    const set = setOf([devQa(), review(), verify([])])
    expect({review: set.review?.kind, devQas: set.devQas.length, verifies: set.verifies.length, trace: set.trace}).toEqual({review: 'code-review', devQas: 1, verifies: 1, trace: undefined})
  })

  test('re-runs the schema, so a hand-built contribution cannot skip the cross-field rules', () => {
    expect(refusalOf([devQa({uploads: []})])).toBe('contribution 1: findings.0.evidence: /tmp/qa/blank.png is not in uploads')
  })

  test('refuses an empty list', () => {
    expect(refusalOf([])).toContain('no contributions')
  })

  test('refuses mixed heads, naming both', () => {
    const reason = refusalOf([review(), devQa({head: OTHER_HEAD})])
    expect(reason).toContain(HEAD)
    expect(reason).toContain(OTHER_HEAD)
  })

  test('refuses mixed pull requests, naming both', () => {
    const reason = refusalOf([review(), devQa({pr: 99})])
    expect(reason).toContain('1234')
    expect(reason).toContain('99')
  })

  test('refuses a merge with only round-two contributions', () => {
    expect(refusalOf([verify([])])).toContain('no code-review or dev-qa')
  })

  test('refuses two code-review contributions', () => {
    expect(refusalOf([review(), review()])).toContain('2 code-review')
  })

  test('refuses two code-trace contributions', () => {
    expect(refusalOf([devQa(), trace([]), trace([])])).toContain('2 code-trace')
  })

  test('refuses two Dev QA contributions without distinct platforms, naming the collision', () => {
    expect(refusalOf([devQa({platform: 'macOS'}), devQa({platform: 'macOS'})])).toContain('macOS')
    expect(refusalOf([devQa(), devQa()])).toContain('platform')
  })

  test('refuses two dev-qa-verify contributions for one platform', () => {
    expect(refusalOf([review(), verify([], {platform: 'macOS'}), verify([], {platform: 'macOS'})])).toContain('macOS')
  })

  test('refuses a verify verdict for an unknown id, naming the id', () => {
    const verdict: VerifyVerdict = {id: 'R9', result: 'not-reachable', evidence: [], note: 'Nothing.'}
    expect(refusalOf([review(), verify([verdict])])).toContain('R9')
  })

  test('refuses a verify verdict for a review finding that routeFindings does not route', () => {
    const verdict: VerifyVerdict = {id: 'R2', result: 'not-reachable', evidence: [], note: 'Nothing to see.'}
    expect(refusalOf([review(), verify([verdict])])).toContain('R2')
  })

  test('refuses a trace verdict for an unknown id, naming the id', () => {
    const verdict: TraceVerdict = {id: 'Q1@Linux', result: 'inconclusive', body: 'Nothing.'}
    expect(refusalOf([devQa({platform: 'macOS'}), trace([verdict])])).toContain('Q1@Linux')
  })

  test('refuses a trace verdict for a Dev QA code finding that routeFindings does not route', () => {
    const untested: Finding = {id: 'Q2', kind: 'code', title: 'Step 3 not run', body: 'No test machine.'}
    const verdict: TraceVerdict = {id: 'Q2', result: 'inconclusive', body: 'Nothing.'}
    expect(refusalOf([devQa({findings: [blankPage(), untested]}), trace([verdict])])).toContain('Q2')
  })

  test('refuses a duplicateOf that names no review finding, naming the id', () => {
    const verdict: TraceVerdict = {id: 'Q1', result: 'root-cause', body: 'Same cause.', duplicateOf: 'R7'}
    expect(refusalOf([review(), devQa(), trace([verdict])])).toContain('R7')
  })

  test('refuses a platform that contains @, so a qualified id cannot collide', () => {
    expect(refusalOf([devQa({platform: 'a@b'}), devQa({platform: 'b'})])).toContain('platform')
  })
})
