import type {CodeReviewContribution, CodeTraceContribution, Contribution, DevQaContribution, DevQaVerifyContribution, Finding, TraceVerdict, VerifyVerdict} from '../contribution'
import {validateContributions, type ContributionSet} from '../contribution-set'
import type {ReviewPayload} from '../payload'
import {synthesize} from '../synthesize'

export const HEAD = 'a'.repeat(40)
export const OTHER_HEAD = 'b'.repeat(40)
export const ATTRIBUTION = 'Posted by Claude on behalf of @someone'
export const ZONES = '# Pre-Claude zone\n\n# Claude zone'

export const staleToken: Finding = {
  id: 'R1',
  kind: 'code',
  title: 'Stale token',
  body: 'The refresh races the expiry.',
  anchor: {path: 'src/a.ts', line: 42, side: 'RIGHT'},
  behavior: 'The user is signed out after one hour.'
}

export const unusedImport: Finding = {id: 'R2', kind: 'code', title: 'Unused import', body: 'The import is unused.'}

export const blankPage = (evidence = '/tmp/qa/blank.png'): Finding => ({
  id: 'Q1',
  kind: 'behavior',
  title: 'Blank page',
  body: 'The page is blank.',
  behavior: 'The page is blank.',
  evidence: [evidence]
})

export const review = (overrides: Partial<CodeReviewContribution> = {}): CodeReviewContribution => ({
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

export const devQa = (overrides: Partial<DevQaContribution> = {}): DevQaContribution => ({
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

export const windowsDevQa = (overrides: Partial<DevQaContribution> = {}): DevQaContribution =>
  devQa({platform: 'Windows', findings: [blankPage('/tmp/qa/win.png')], uploads: ['/tmp/qa/win.png'], body: 'Blank on Windows.\n\n/tmp/qa/win.png', ...overrides})

export const verify = (verdicts: VerifyVerdict[], overrides: Partial<DevQaVerifyContribution> = {}): DevQaVerifyContribution => ({
  version: 1,
  kind: 'dev-qa-verify',
  pr: 1234,
  head: HEAD,
  verdicts,
  uploads: verdicts.flatMap(verdict => verdict.evidence),
  handback: [],
  ...overrides
})

export const trace = (verdicts: TraceVerdict[]): CodeTraceContribution => ({
  version: 1,
  kind: 'code-trace',
  pr: 1234,
  head: HEAD,
  verdicts,
  uploads: [],
  handback: []
})

export function setOf(contributions: Contribution[]): ContributionSet {
  const result = validateContributions(contributions)
  if (!result.ok) throw new Error(`validateContributions refused: ${result.reason}`)
  return result.value
}

export function refusalOf(contributions: Contribution[]): string {
  const result = validateContributions(contributions)
  return result.ok ? 'validated' : result.reason
}

export function payloadOf(contributions: Contribution[], attribution: string | undefined = ATTRIBUTION): ReviewPayload {
  return synthesize(setOf(contributions), attribution)
}

export const attributed = (body: string) => `${ATTRIBUTION}\n\n${body}`
