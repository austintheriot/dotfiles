import {describe, expect, test} from 'bun:test'
import type {CodeReviewContribution, CodeTraceContribution, DevQaContribution, DevQaVerifyContribution, Finding} from './contribution'
import {notRun, parseContribution} from './contribution'

const HEAD = 'a'.repeat(40)

const reviewJson: CodeReviewContribution = {
  version: 1,
  kind: 'code-review',
  pr: 1234,
  head: HEAD,
  body: 'Two defects in the refresh path.',
  comments: [{path: 'src/a.ts', line: 42, side: 'RIGHT', startLine: 40, body: 'The token expires first.', finding: 'R1'}],
  findings: [
    {id: 'R1', kind: 'code', title: 'Stale token', body: 'The refresh races the expiry.', anchor: {path: 'src/a.ts', line: 42, side: 'RIGHT'}, behavior: 'The user is signed out.'}
  ],
  uploads: [],
  handback: []
}

const blankPageFinding: Finding = {id: 'Q1', kind: 'behavior', title: 'Blank page', body: 'The page is blank.', behavior: 'The page is blank.', evidence: ['/tmp/qa/blank.png']}

const devQaJson: DevQaContribution = {
  version: 1,
  kind: 'dev-qa',
  pr: 1234,
  head: HEAD,
  platform: 'macOS',
  status: 'failed',
  reason: 'Step 2 failed.',
  body: 'Step 2 shows a blank page. /tmp/qa/blank.png',
  comments: [],
  findings: [blankPageFinding],
  uploads: ['/tmp/qa/blank.png'],
  handback: ['The deployed commit is the head.']
}

const verifyJson: DevQaVerifyContribution = {
  version: 1,
  kind: 'dev-qa-verify',
  pr: 1234,
  head: HEAD,
  verdicts: [{id: 'R1', result: 'reproduced', evidence: ['/tmp/qa/r1.mp4'], note: 'Waited one hour and the app signed out.'}],
  uploads: ['/tmp/qa/r1.mp4'],
  handback: []
}

const traceJson: CodeTraceContribution = {
  version: 1,
  kind: 'code-trace',
  pr: 1234,
  head: HEAD,
  verdicts: [{id: 'Q1', result: 'root-cause', anchor: {path: 'src/b.ts', line: 7, side: 'RIGHT'}, body: 'The loader returns early.'}],
  uploads: [],
  handback: []
}

const parse = (json: unknown) => parseContribution(JSON.stringify(json))

const reasonOf = (json: unknown) => {
  const parsed = parse(json)
  return parsed.ok ? 'parsed' : parsed.reason
}

describe('parseContribution', () => {
  test('parses a code-review contribution', () => {
    expect(parse(reviewJson)).toEqual({ok: true, value: reviewJson})
  })

  test('parses a dev-qa contribution', () => {
    expect(parse(devQaJson)).toEqual({ok: true, value: devQaJson})
  })

  test('parses a dev-qa-verify contribution', () => {
    expect(parse(verifyJson)).toEqual({ok: true, value: verifyJson})
  })

  test('parses a code-trace contribution', () => {
    expect(parse(traceJson)).toEqual({ok: true, value: traceJson})
  })

  test('rejects a missing head, naming the field', () => {
    const {head: _head, ...withoutHead} = reviewJson
    expect(reasonOf(withoutHead)).toContain('head')
  })

  test('rejects a head that is not a full commit SHA', () => {
    expect(reasonOf({...reviewJson, head: 'abc1234'})).toContain('head')
  })

  test('rejects an unknown kind, naming the field', () => {
    expect(reasonOf({...reviewJson, kind: 'lint'})).toContain('kind')
  })

  test('rejects a dev-qa contribution with no status, naming the field', () => {
    const {status: _status, ...withoutStatus} = devQaJson
    expect(reasonOf(withoutStatus)).toContain('status')
  })

  test('rejects text that is not JSON', () => {
    expect(parseContribution('{')).toEqual({ok: false, reason: 'not JSON'})
  })

  test('rejects an unknown key, naming it', () => {
    expect(reasonOf({...reviewJson, severity: 'major'})).toContain('severity')
  })

  test('rejects null for an optional field', () => {
    expect(reasonOf({...devQaJson, platform: null})).toContain('platform')
  })

  test('rejects a dev-qa contribution that did not pass and gives no reason', () => {
    const {reason: _reason, ...withoutReason} = devQaJson
    expect(reasonOf(withoutReason)).toContain('reason')
  })

  test('accepts a passed dev-qa contribution with no reason', () => {
    const {reason: _reason, ...withoutReason} = devQaJson
    expect(parse({...withoutReason, status: 'passed'}).ok).toBe(true)
  })

  test('rejects evidence that is not in uploads', () => {
    expect(reasonOf({...devQaJson, uploads: []})).toContain('/tmp/qa/blank.png')
  })

  test('rejects verdict evidence that is not in uploads', () => {
    expect(reasonOf({...verifyJson, uploads: []})).toContain('/tmp/qa/r1.mp4')
  })

  test('rejects a comment whose finding is not a finding id', () => {
    const comments = [{...reviewJson.comments[0], finding: 'R9'}]
    expect(reasonOf({...reviewJson, comments})).toContain('R9')
  })

  test('rejects two findings with one id', () => {
    const findings = [reviewJson.findings[0], reviewJson.findings[0]]
    expect(reasonOf({...reviewJson, findings})).toContain('R1')
  })

  test('rejects a behavior finding with no behavior', () => {
    const {behavior: _behavior, ...withoutBehavior} = blankPageFinding
    expect(reasonOf({...devQaJson, findings: [withoutBehavior]})).toContain('behavior')
  })

  test('rejects a behavior finding with empty evidence', () => {
    const findings = [{...blankPageFinding, evidence: []}]
    expect(reasonOf({...devQaJson, findings})).toContain('evidence')
  })

  test('rejects a comment whose startLine is after its line', () => {
    const comments = [{...reviewJson.comments[0], startLine: 50}]
    expect(reasonOf({...reviewJson, comments})).toContain('startLine')
  })

  test('rejects duplicateOf on an inconclusive trace verdict', () => {
    const verdicts = [{id: 'Q1', result: 'inconclusive', body: 'No cause found.', duplicateOf: 'R1'}]
    expect(reasonOf({...traceJson, verdicts})).toContain('duplicateOf')
  })

  test('rejects a body on a round-two contribution', () => {
    expect(reasonOf({...verifyJson, body: 'text'})).toContain('body')
  })

  test('accepts a platform on a dev-qa-verify contribution', () => {
    expect(parse({...verifyJson, platform: 'Windows'})).toEqual({ok: true, value: {...verifyJson, platform: 'Windows'}})
  })

  test('accepts a not-run code-review contribution with a reason', () => {
    const json = {...reviewJson, status: 'not-run', reason: 'The diff did not load.', body: 'Code review: not run. The diff did not load.', comments: [], findings: []}
    expect(parse(json).ok).toBe(true)
  })

  test('accepts a completed code-review contribution', () => {
    expect(parse({...reviewJson, status: 'completed'}).ok).toBe(true)
  })

  test('rejects a not-run code-review contribution with no reason', () => {
    const json = {...reviewJson, status: 'not-run', comments: [], findings: []}
    expect(reasonOf(json)).toContain('reason')
  })

  test('rejects a not-run code-review contribution that has findings', () => {
    expect(reasonOf({...reviewJson, status: 'not-run', reason: 'The diff did not load.'})).toContain('findings')
  })
})

describe('parseContribution id grammar', () => {
  test('rejects a code-review finding id that does not start with R', () => {
    const findings = [{...reviewJson.findings[0], id: 'Q1'}]
    const comments = [{...reviewJson.comments[0], finding: 'Q1'}]
    expect(reasonOf({...reviewJson, findings, comments})).toContain('must start with R')
  })

  test('rejects a dev-qa finding id that does not start with Q', () => {
    expect(reasonOf({...devQaJson, findings: [{...blankPageFinding, id: 'R1'}]})).toContain('must start with Q')
  })

  test('rejects a finding id outside the grammar', () => {
    expect(reasonOf({...devQaJson, findings: [{...blankPageFinding, id: 'Q1a'}]})).toContain('findings.0.id')
  })

  test('rejects a platform that contains @', () => {
    expect(reasonOf({...devQaJson, platform: 'mac@os'})).toContain('platform')
    expect(reasonOf({...verifyJson, platform: 'mac@os'})).toContain('platform')
  })

  test('rejects a verify verdict id that is not a review id', () => {
    const verdicts = [{id: 'Q1', result: 'not-reachable', evidence: [], note: 'Nothing.'}]
    expect(reasonOf({...verifyJson, verdicts})).toContain('verdicts.0.id')
  })

  test.each(['R1', 'Q1@a@b', 'Q1@'])('rejects the trace verdict id %s', id => {
    const verdicts = [{id, result: 'inconclusive', body: 'Nothing.'}]
    expect(reasonOf({...traceJson, verdicts})).toContain('verdicts.0.id')
  })

  test('accepts a trace verdict id qualified with a platform', () => {
    const verdicts = [{id: 'Q1@macOS', result: 'inconclusive', body: 'Nothing.'}]
    expect(parse({...traceJson, verdicts}).ok).toBe(true)
  })

  test('rejects a duplicateOf that is not a review id', () => {
    const verdicts = [{id: 'Q1', result: 'root-cause', body: 'Same.', duplicateOf: 'Q2'}]
    expect(reasonOf({...traceJson, verdicts})).toContain('duplicateOf')
  })

  test('rejects a not-run dev-qa contribution that has findings', () => {
    expect(reasonOf({...devQaJson, status: 'not-run'})).toBe('findings: a not-run dev-qa contribution has no findings')
  })

  test('rejects a not-run dev-qa contribution that has comments', () => {
    const comments = [{path: 'src/a.ts', line: 1, side: 'RIGHT', body: 'Draft.', finding: 'Q1'}]
    expect(reasonOf({...devQaJson, status: 'not-run', findings: [], uploads: [], comments})).toBe('comments: a not-run dev-qa contribution has no comments')
  })
})

describe('notRun', () => {
  test('builds a code-review contribution that parses', () => {
    const contribution = notRun({kind: 'code-review', pr: 1234, head: HEAD, reason: 'The diff did not load.'})
    expect(contribution).toEqual({
      version: 1,
      kind: 'code-review',
      pr: 1234,
      head: HEAD,
      status: 'not-run',
      reason: 'The diff did not load.',
      body: 'Code review: not run. The diff did not load.',
      comments: [],
      findings: [],
      uploads: [],
      handback: []
    })
    expect(parse(contribution)).toEqual({ok: true, value: contribution})
  })

  test('builds a dev-qa contribution with its platform that parses', () => {
    const contribution = notRun({kind: 'dev-qa', pr: 1234, head: HEAD, platform: 'Windows', reason: 'No test machine.'})
    expect(contribution).toEqual({
      version: 1,
      kind: 'dev-qa',
      pr: 1234,
      head: HEAD,
      platform: 'Windows',
      status: 'not-run',
      reason: 'No test machine.',
      body: 'Dev QA (Windows): not run. No test machine.',
      comments: [],
      findings: [],
      uploads: [],
      handback: []
    })
    expect(parse(contribution)).toEqual({ok: true, value: contribution})
  })

  test('builds a dev-qa contribution with no platform', () => {
    const contribution = notRun({kind: 'dev-qa', pr: 1234, head: HEAD, reason: 'No test machine.'})
    expect(contribution).not.toHaveProperty('platform')
    expect(contribution).toMatchObject({body: 'Dev QA: not run. No test machine.'})
  })
})
