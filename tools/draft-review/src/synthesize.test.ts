import {describe, expect, test} from 'bun:test'
import type {CodeReviewContribution, CodeTraceContribution, Contribution, DevQaContribution, DevQaVerifyContribution, Finding, TraceVerdict, VerifyVerdict} from './contribution'
import {notRun} from './contribution'
import {routeFindings, synthesize} from './synthesize'

const HEAD = 'a'.repeat(40)
const OTHER_HEAD = 'b'.repeat(40)
const ATTRIBUTION = 'Posted by Claude on behalf of @someone'
const ZONES = '# Pre-Claude zone\n\n# Claude zone'

const staleToken: Finding = {
  id: 'R1',
  kind: 'code',
  title: 'Stale token',
  body: 'The refresh races the expiry.',
  anchor: {path: 'src/a.ts', line: 42, side: 'RIGHT'},
  behavior: 'The user is signed out after one hour.'
}

const unusedImport: Finding = {id: 'R2', kind: 'code', title: 'Unused import', body: 'The import is unused.'}

const blankPage = (evidence = '/tmp/qa/blank.png'): Finding => ({
  id: 'Q1',
  kind: 'behavior',
  title: 'Blank page',
  body: 'The page is blank.',
  behavior: 'The page is blank.',
  evidence: [evidence]
})

const review = (overrides: Partial<CodeReviewContribution> = {}): CodeReviewContribution => ({
  version: 1,
  kind: 'code-review',
  pr: 1234,
  head: HEAD,
  body: 'The refresh path has one defect.',
  comments: [{path: 'src/a.ts', line: 42, side: 'RIGHT', body: 'The token expires before the refresh.', finding: 'R1'}],
  findings: [staleToken, unusedImport],
  uploads: [],
  handback: [],
  ...overrides
})

const devQa = (overrides: Partial<DevQaContribution> = {}): DevQaContribution => ({
  version: 1,
  kind: 'dev-qa',
  pr: 1234,
  head: HEAD,
  status: 'failed',
  reason: 'Step 2 failed.',
  body: 'Step 2 shows a blank page.\n\n/tmp/qa/blank.png',
  comments: [],
  findings: [blankPage()],
  uploads: ['/tmp/qa/blank.png'],
  handback: [],
  ...overrides
})

const verify = (verdicts: VerifyVerdict[], overrides: Partial<DevQaVerifyContribution> = {}): DevQaVerifyContribution => ({
  version: 1,
  kind: 'dev-qa-verify',
  pr: 1234,
  head: HEAD,
  verdicts,
  uploads: verdicts.flatMap(verdict => verdict.evidence),
  handback: [],
  ...overrides
})

const trace = (verdicts: TraceVerdict[]): CodeTraceContribution => ({
  version: 1,
  kind: 'code-trace',
  pr: 1234,
  head: HEAD,
  verdicts,
  uploads: [],
  handback: []
})

const payloadOf = (contributions: Contribution[]) => {
  const result = synthesize(contributions, ATTRIBUTION)
  if (!result.ok) throw new Error(`synthesize refused: ${result.reason}`)
  return result.payload
}

const reasonOf = (contributions: Contribution[]) => {
  const result = synthesize(contributions, ATTRIBUTION)
  return result.ok ? 'synthesized' : result.reason
}

const attributed = (body: string) => `${ATTRIBUTION}\n\n${body}`

describe('routeFindings', () => {
  test('sends review findings with a behavior to verify and skips the others', () => {
    expect(routeFindings([review()]).toVerify).toEqual([staleToken])
  })

  test('sends Dev QA behavior findings to trace and routes code findings nowhere', () => {
    const untested: Finding = {id: 'Q2', kind: 'code', title: 'Step 3 not run', body: 'No test machine for step 3.'}
    expect(routeFindings([devQa({findings: [blankPage(), untested]})])).toEqual({toVerify: [], toTrace: [blankPage()]})
  })

  test('qualifies Dev QA ids with their platform and keeps two platforms apart', () => {
    const contributions = [
      devQa({platform: 'macOS'}),
      devQa({platform: 'Windows', findings: [blankPage('/tmp/qa/win.png')], uploads: ['/tmp/qa/win.png'], body: 'Blank. /tmp/qa/win.png'})
    ]
    expect(routeFindings(contributions).toTrace.map(finding => finding.id)).toEqual(['Q1@macOS', 'Q1@Windows'])
  })

  test('never qualifies review ids', () => {
    expect(routeFindings([review(), devQa({platform: 'macOS'})]).toVerify.map(finding => finding.id)).toEqual(['R1'])
  })
})

describe('synthesize', () => {
  test('builds a review-only payload under the zone headers and the attribution line', () => {
    expect(payloadOf([review()])).toEqual({
      body: `${ZONES}\n\n${ATTRIBUTION}\n\nThe refresh path has one defect.`,
      comments: [{path: 'src/a.ts', line: 42, side: 'RIGHT', body: attributed('The token expires before the refresh.')}],
      uploads: [],
      handback: []
    })
  })

  test('adds no attribution line when none is defined', () => {
    const result = synthesize([review()], undefined)
    if (!result.ok) throw new Error(`synthesize refused: ${result.reason}`)
    const payload = result.payload
    expect(payload.body).toBe(`${ZONES}\n\nThe refresh path has one defect.`)
    expect(payload.comments[0]?.body).toBe('The token expires before the refresh.')
  })

  test('keeps the startLine of a multi-line comment', () => {
    const comments = [{path: 'src/a.ts', line: 42, side: 'RIGHT' as const, startLine: 40, body: 'Range.', finding: 'R1'}]
    expect(payloadOf([review({comments})]).comments[0]).toMatchObject({startLine: 40, line: 42})
  })

  test('builds a Dev QA-only payload', () => {
    expect(payloadOf([devQa({handback: ['The deployed commit is the head.']})])).toEqual({
      body: `${ZONES}\n\n${ATTRIBUTION}\n\nStep 2 shows a blank page.\n\n/tmp/qa/blank.png`,
      comments: [],
      uploads: ['/tmp/qa/blank.png'],
      handback: ['The deployed commit is the head.']
    })
  })

  test('puts the review body before the Dev QA part', () => {
    const payload = payloadOf([devQa(), review()])
    expect(payload.body).toBe(`${ZONES}\n\n${ATTRIBUTION}\n\nThe refresh path has one defect.\n\nStep 2 shows a blank page.\n\n/tmp/qa/blank.png`)
  })

  test('gives each of two Dev QA platforms its own heading', () => {
    const windows = devQa({platform: 'Windows', status: 'passed', body: 'Every step passed.', findings: [], uploads: []})
    const {reason: _reason, ...passed} = windows
    const payload = payloadOf([review(), devQa({platform: 'macOS'}), passed])
    expect(payload.body).toBe(
      `${ZONES}\n\n${ATTRIBUTION}\n\nThe refresh path has one defect.\n\n### macOS\n\nStep 2 shows a blank page.\n\n/tmp/qa/blank.png\n\n### Windows\n\nEvery step passed.`
    )
  })

  test('writes a not-run Dev QA part as one line', () => {
    const payload = payloadOf([review(), notRun({kind: 'dev-qa', pr: 1234, head: HEAD, platform: 'Windows', reason: 'No test machine.'})])
    expect(payload.body).toBe(`${ZONES}\n\n${ATTRIBUTION}\n\nThe refresh path has one defect.\n\nDev QA (Windows): not run. No test machine.`)
  })

  test('writes a not-run code review as one line', () => {
    const payload = payloadOf([notRun({kind: 'code-review', pr: 1234, head: HEAD, reason: 'The diff did not load.'}), devQa()])
    expect(payload.body).toBe(`${ZONES}\n\n${ATTRIBUTION}\n\nCode review: not run. The diff did not load.\n\nStep 2 shows a blank page.\n\n/tmp/qa/blank.png`)
  })

  test('a reproduced review finding keeps its comment, gains the evidence, and gets one Dev QA pointer line', () => {
    const verdict: VerifyVerdict = {id: 'R1', result: 'reproduced', evidence: ['/tmp/qa/r1.mp4'], note: 'Signed out after one hour.'}
    const payload = payloadOf([review(), devQa({platform: 'macOS'}), verify([verdict], {platform: 'macOS'})])
    expect(payload.comments[0]?.body).toBe(attributed('The token expires before the refresh.\n\nReproduced on the deployed build (macOS).\n\n/tmp/qa/r1.mp4'))
    expect(payload.body.endsWith('Reproduced on the deployed build (macOS): Stale token. See the inline comment on `src/a.ts` line 42.')).toBe(true)
    expect(payload.body.match(/Reproduced on the deployed build/g)?.length).toBe(1)
    expect(payload.uploads).toEqual(['/tmp/qa/blank.png', '/tmp/qa/r1.mp4'])
  })

  test('a review finding reproduced on two platforms lists both in input order', () => {
    const mac: VerifyVerdict = {id: 'R1', result: 'reproduced', evidence: ['/tmp/qa/mac.mp4'], note: 'Signed out.'}
    const win: VerifyVerdict = {id: 'R1', result: 'reproduced', evidence: ['/tmp/qa/win.mp4'], note: 'Signed out.'}
    const payload = payloadOf([review(), verify([win], {platform: 'Windows'}), verify([mac], {platform: 'macOS'})])
    expect(payload.comments[0]?.body).toBe(attributed('The token expires before the refresh.\n\nReproduced on the deployed build (Windows, macOS).\n\n/tmp/qa/win.mp4\n\n/tmp/qa/mac.mp4'))
  })

  test('a review finding reproduced on one platform and not on another counts as reproduced', () => {
    const mac: VerifyVerdict = {id: 'R1', result: 'reproduced', evidence: ['/tmp/qa/mac.mp4'], note: 'Signed out.'}
    const win: VerifyVerdict = {id: 'R1', result: 'not-reproduced', evidence: ['/tmp/qa/win.png'], note: 'Still signed in after one hour.'}
    const payload = payloadOf([review(), verify([mac], {platform: 'macOS'}), verify([win], {platform: 'Windows'})])
    expect(payload.comments[0]?.body).toContain('Reproduced on the deployed build (macOS).')
    expect(payload.handback).toEqual(['R1 not-reproduced (Windows): Still signed in after one hour.'])
  })

  test('a reproduced review finding with no comment carries its evidence in the pointer', () => {
    const verdict: VerifyVerdict = {id: 'R1', result: 'reproduced', evidence: ['/tmp/qa/r1.mp4'], note: 'Signed out.'}
    const payload = payloadOf([review({comments: []}), verify([verdict])])
    expect(payload.body.endsWith('Reproduced on the deployed build: Stale token.\n\n/tmp/qa/r1.mp4')).toBe(true)
    expect(payload.uploads).toEqual(['/tmp/qa/r1.mp4'])
  })

  test('a not-reproduced review finding keeps its comment unchanged and goes to hand-back only', () => {
    const verdict: VerifyVerdict = {id: 'R1', result: 'not-reproduced', evidence: ['/tmp/qa/r1.png'], note: 'Still signed in.'}
    const alone = payloadOf([review(), devQa()])
    const payload = payloadOf([review(), devQa(), verify([verdict])])
    expect({body: payload.body, comments: payload.comments, uploads: payload.uploads}).toEqual({body: alone.body, comments: alone.comments, uploads: alone.uploads})
    expect(payload.handback).toEqual(['R1 not-reproduced: Still signed in.'])
  })

  test('a not-reachable review finding keeps its comment unchanged and goes to hand-back only', () => {
    const verdict: VerifyVerdict = {id: 'R1', result: 'not-reachable', evidence: [], note: 'No account with an expired token.'}
    const alone = payloadOf([review(), devQa()])
    const payload = payloadOf([review(), devQa(), verify([verdict])])
    expect({body: payload.body, comments: payload.comments}).toEqual({body: alone.body, comments: alone.comments})
    expect(payload.handback).toEqual(['R1 not-reachable: No account with an expired token.'])
  })

  test('a root-cause trace adds an inline comment at the trace anchor and a Dev QA line that points to it', () => {
    const verdict: TraceVerdict = {id: 'Q1', result: 'root-cause', anchor: {path: 'src/b.ts', line: 7, side: 'RIGHT'}, body: 'The loader returns early.'}
    const payload = payloadOf([review(), devQa(), trace([verdict])])
    expect(payload.comments).toEqual([
      {path: 'src/a.ts', line: 42, side: 'RIGHT', body: attributed('The token expires before the refresh.')},
      {path: 'src/b.ts', line: 7, side: 'RIGHT', body: attributed('The loader returns early.')}
    ])
    expect(payload.body.endsWith('/tmp/qa/blank.png\n\nBlank page: the cause is in `src/b.ts` line 7. See the inline comment there.')).toBe(true)
  })

  test('a root-cause trace with no anchor puts the cause in the Dev QA part', () => {
    const verdict: TraceVerdict = {id: 'Q1', result: 'root-cause', body: 'The feature flag is off in the build.'}
    const payload = payloadOf([devQa(), trace([verdict])])
    expect(payload.comments).toEqual([])
    expect(payload.body.endsWith('Blank page: the cause is in the code.\n\nThe feature flag is off in the build.')).toBe(true)
  })

  test('a root-cause trace that duplicates a review finding adds no comment and gives the review comment the Dev QA evidence', () => {
    const verdict: TraceVerdict = {id: 'Q1@macOS', result: 'root-cause', body: 'Same race as the token refresh.', duplicateOf: 'R1'}
    const payload = payloadOf([review(), devQa({platform: 'macOS'}), trace([verdict])])
    expect(payload.comments).toEqual([
      {path: 'src/a.ts', line: 42, side: 'RIGHT', body: attributed('The token expires before the refresh.\n\nSeen on the deployed build (macOS): Blank page.\n\n/tmp/qa/blank.png')}
    ])
  })

  test('a root-cause trace that duplicates a review finding with no comment adds a Dev QA line that names it', () => {
    const verdict: TraceVerdict = {id: 'Q1', result: 'root-cause', body: 'Same race.', duplicateOf: 'R1'}
    const payload = payloadOf([review({comments: []}), devQa(), trace([verdict])])
    expect(payload.comments).toEqual([])
    expect(payload.body.endsWith('Blank page: same cause as the code finding "Stale token".')).toBe(true)
  })

  test('drops the inline comments of a Dev QA part that did not run', () => {
    const skipped = devQa({status: 'not-run', reason: 'No machine.', comments: [{path: 'src/a.ts', line: 1, side: 'RIGHT', body: 'Draft.', finding: 'Q1'}]})
    expect(payloadOf([skipped]).comments).toEqual([])
  })

  test.each(['no-code-cause', 'inconclusive'] as const)('a %s trace leaves the payload unchanged', result => {
    const verdict: TraceVerdict = {id: 'Q1', result, body: 'Nothing in the diff explains the symptom.'}
    expect(payloadOf([review(), devQa(), trace([verdict])])).toEqual(payloadOf([review(), devQa()]))
  })

  test('no round-two contribution leaves the round-one parts as written', () => {
    const payload = payloadOf([review(), devQa()])
    expect(payload.comments).toEqual([{path: 'src/a.ts', line: 42, side: 'RIGHT', body: attributed('The token expires before the refresh.')}])
    expect(payload.handback).toEqual([])
  })

  test('refuses a verify verdict for an unknown id, naming the id', () => {
    const verdict: VerifyVerdict = {id: 'R9', result: 'not-reachable', evidence: [], note: 'Nothing.'}
    expect(reasonOf([review(), verify([verdict])])).toContain('R9')
  })

  test('refuses a trace verdict for an unknown id, naming the id', () => {
    const verdict: TraceVerdict = {id: 'Q1@Linux', result: 'inconclusive', body: 'Nothing.'}
    expect(reasonOf([devQa({platform: 'macOS'}), trace([verdict])])).toContain('Q1@Linux')
  })

  test('refuses a duplicateOf that names no review finding, naming the id', () => {
    const verdict: TraceVerdict = {id: 'Q1', result: 'root-cause', body: 'Same cause.', duplicateOf: 'R7'}
    expect(reasonOf([review(), devQa(), trace([verdict])])).toContain('R7')
  })

  test('refuses mixed heads, naming both', () => {
    const reason = reasonOf([review(), devQa({head: OTHER_HEAD})])
    expect(reason).toContain(HEAD)
    expect(reason).toContain(OTHER_HEAD)
  })

  test('refuses mixed pull requests, naming both', () => {
    const reason = reasonOf([review(), devQa({pr: 99})])
    expect(reason).toContain('1234')
    expect(reason).toContain('99')
  })

  test('refuses two Dev QA contributions without distinct platforms, naming the collision', () => {
    expect(reasonOf([devQa({platform: 'macOS'}), devQa({platform: 'macOS'})])).toContain('macOS')
    expect(reasonOf([devQa(), devQa()])).toContain('platform')
  })

  test('refuses an empty list', () => {
    expect(reasonOf([])).toContain('no contributions')
  })

  test('lists a file cited twice as one upload', () => {
    const payload = payloadOf([review({body: 'See /tmp/qa/blank.png.', uploads: ['/tmp/qa/blank.png']}), devQa()])
    expect(payload.uploads).toEqual(['/tmp/qa/blank.png'])
  })

  test('orders uploads by first citation in the payload', () => {
    const payload = payloadOf([review({body: 'See /tmp/b.png and /tmp/a.png.', uploads: ['/tmp/a.png', '/tmp/b.png']}), devQa()])
    expect(payload.uploads).toEqual(['/tmp/b.png', '/tmp/a.png', '/tmp/qa/blank.png'])
  })

  test('keeps the ids of two platforms apart when a trace answers one of them', () => {
    const windows = devQa({platform: 'Windows', findings: [blankPage('/tmp/qa/win.png')], uploads: ['/tmp/qa/win.png'], body: 'Blank on Windows.\n\n/tmp/qa/win.png'})
    const verdict: TraceVerdict = {id: 'Q1@Windows', result: 'root-cause', anchor: {path: 'src/win.ts', line: 3, side: 'RIGHT'}, body: 'The path separator is wrong.'}
    const payload = payloadOf([devQa({platform: 'macOS'}), windows, trace([verdict])])
    expect(payload.body).toBe(
      `${ZONES}\n\n${ATTRIBUTION}\n\n### macOS\n\nStep 2 shows a blank page.\n\n/tmp/qa/blank.png\n\n### Windows\n\nBlank on Windows.\n\n/tmp/qa/win.png\n\nBlank page: the cause is in \`src/win.ts\` line 3. See the inline comment there.`
    )
  })

  test('collects hand-back lines from every contribution in input order', () => {
    const verdict: VerifyVerdict = {id: 'R1', result: 'not-reachable', evidence: [], note: 'No account.'}
    const payload = payloadOf([review({handback: ['Review line.']}), devQa({handback: ['Dev QA line.']}), verify([verdict], {handback: ['Verify line.']})])
    expect(payload.handback).toEqual(['Review line.', 'Dev QA line.', 'Verify line.', 'R1 not-reachable: No account.'])
  })
})
