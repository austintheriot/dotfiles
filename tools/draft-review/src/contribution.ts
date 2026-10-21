import {z} from 'zod'
import {duplicates} from './lists'
import type {Parsed} from './parsed'

const Text = z.string().min(1)
const Line = z.number().int().positive()
const Side = z.enum(['LEFT', 'RIGHT'])
const LocalPaths = z.array(Text)
const Platform = Text.regex(/^[^@]+$/, 'a platform label cannot contain @')

export const AnchorSchema = z.object({path: Text, line: Line, side: Side}).strict()

export const ContributionCommentSchema = z
  .object({path: Text, line: Line, side: Side, startLine: Line.optional(), body: Text, finding: Text})
  .strict()

export const FindingSchema = z
  .object({
    id: z.string().regex(/^[RQ][0-9]+$/, 'a finding id is R or Q followed by digits'),
    kind: z.enum(['code', 'behavior']),
    title: Text,
    body: Text,
    anchor: AnchorSchema.optional(),
    behavior: Text.optional(),
    evidence: LocalPaths.optional()
  })
  .strict()

export const VerifyVerdictSchema = z
  .object({
    id: z.string().regex(/^R[0-9]+$/, 'a dev-qa-verify verdict id is a review finding id, R followed by digits'),
    result: z.enum(['reproduced', 'not-reproduced', 'not-reachable']),
    evidence: LocalPaths,
    note: Text
  })
  .strict()

export const TraceVerdictSchema = z
  .object({
    id: z.string().regex(/^Q[0-9]+(@[^@]+)?$/, 'a code-trace verdict id is a Dev QA finding id, Q followed by digits and an optional @platform'),
    result: z.enum(['root-cause', 'no-code-cause', 'inconclusive']),
    anchor: AnchorSchema.optional(),
    body: Text,
    duplicateOf: z.string().regex(/^R[0-9]+$/, 'duplicateOf is a review finding id, R followed by digits').optional()
  })
  .strict()

const common = {
  version: z.literal(1),
  pr: z.number().int().positive(),
  head: z.string().regex(/^[0-9a-f]{40}$/, 'expected a full 40-character commit SHA'),
  uploads: LocalPaths,
  handback: z.array(Text)
}

const roundOne = {
  body: Text,
  comments: z.array(ContributionCommentSchema),
  findings: z.array(FindingSchema)
}

export const CodeReviewSchema = z
  .object({...common, ...roundOne, kind: z.literal('code-review'), status: z.enum(['completed', 'not-run']).optional(), reason: Text.optional()})
  .strict()

export const DevQaSchema = z
  .object({
    ...common,
    ...roundOne,
    kind: z.literal('dev-qa'),
    platform: Platform.optional(),
    status: z.enum(['passed', 'failed', 'incomplete', 'not-run']),
    reason: Text.optional()
  })
  .strict()

export const DevQaVerifySchema = z
  .object({...common, kind: z.literal('dev-qa-verify'), platform: Platform.optional(), verdicts: z.array(VerifyVerdictSchema)})
  .strict()

export const CodeTraceSchema = z.object({...common, kind: z.literal('code-trace'), verdicts: z.array(TraceVerdictSchema)}).strict()

export type Anchor = z.infer<typeof AnchorSchema>
export type ContributionComment = z.infer<typeof ContributionCommentSchema>
export type Finding = z.infer<typeof FindingSchema>
export type VerifyVerdict = z.infer<typeof VerifyVerdictSchema>
export type TraceVerdict = z.infer<typeof TraceVerdictSchema>
export type CodeReviewContribution = z.infer<typeof CodeReviewSchema>
export type DevQaContribution = z.infer<typeof DevQaSchema>
export type DevQaVerifyContribution = z.infer<typeof DevQaVerifySchema>
export type CodeTraceContribution = z.infer<typeof CodeTraceSchema>
export type Contribution = CodeReviewContribution | DevQaContribution | DevQaVerifyContribution | CodeTraceContribution

type Problem = {path: (string | number)[]; message: string}

function evidenceProblems(contribution: Contribution): Problem[] {
  const uploads = new Set(contribution.uploads)
  const cited = (() => {
    switch (contribution.kind) {
      case 'code-review':
      case 'dev-qa':
        return contribution.findings.map((finding, index) => ({path: ['findings', index, 'evidence'], evidence: finding.evidence ?? []}))
      case 'dev-qa-verify':
        return contribution.verdicts.map((verdict, index) => ({path: ['verdicts', index, 'evidence'], evidence: verdict.evidence}))
      case 'code-trace':
        return []
      default: {
        const unreachable: never = contribution
        return unreachable
      }
    }
  })()
  return cited.flatMap(({path, evidence}) =>
    evidence.filter(file => !uploads.has(file)).map(file => ({path, message: `${file} is not in uploads`}))
  )
}

function findingProblems(contribution: CodeReviewContribution | DevQaContribution): Problem[] {
  const {findings, comments} = contribution
  const letter = contribution.kind === 'code-review' ? 'R' : 'Q'
  const ids = new Set(findings.map(finding => finding.id))
  const wrongLetter = findings.flatMap((finding, index) =>
    finding.id.startsWith(letter) ? [] : [{path: ['findings', index, 'id'], message: `finding id ${finding.id} must start with ${letter} in a ${contribution.kind} contribution`}]
  )
  const repeated = duplicates(findings.map(finding => finding.id)).map(id => ({path: ['findings'], message: `finding id ${id} appears more than once`}))
  const behaviorGaps = findings.flatMap((finding, index): Problem[] => {
    if (finding.kind !== 'behavior') return []
    const missingBehavior = finding.behavior === undefined ? [{path: ['findings', index, 'behavior'], message: 'a behavior finding needs behavior'}] : []
    const missingEvidence = (finding.evidence ?? []).length === 0 ? [{path: ['findings', index, 'evidence'], message: 'a behavior finding needs non-empty evidence'}] : []
    return [...missingBehavior, ...missingEvidence]
  })
  const commentGaps = comments.flatMap((comment, index): Problem[] => [
    ...(ids.has(comment.finding) ? [] : [{path: ['comments', index, 'finding'], message: `${comment.finding} is not a finding id`}]),
    ...(comment.startLine !== undefined && comment.startLine > comment.line
      ? [{path: ['comments', index, 'startLine'], message: 'startLine is after line'}]
      : [])
  ])
  return [...wrongLetter, ...repeated, ...behaviorGaps, ...commentGaps]
}

function notRunProblems(contribution: CodeReviewContribution | DevQaContribution): Problem[] {
  return [
    ...(contribution.reason === undefined ? [{path: ['reason'], message: `a not-run ${contribution.kind} contribution needs a reason`}] : []),
    ...(contribution.findings.length > 0 ? [{path: ['findings'], message: `a not-run ${contribution.kind} contribution has no findings`}] : []),
    ...(contribution.comments.length > 0 ? [{path: ['comments'], message: `a not-run ${contribution.kind} contribution has no comments`}] : [])
  ]
}

const repeatedVerdicts = (ids: string[]): Problem[] => duplicates(ids).map(id => ({path: ['verdicts'], message: `verdict id ${id} appears more than once`}))

function kindProblems(contribution: Contribution): Problem[] {
  switch (contribution.kind) {
    case 'code-review':
      return contribution.status === 'not-run' ? notRunProblems(contribution) : []
    case 'dev-qa':
      if (contribution.status === 'not-run') return notRunProblems(contribution)
      return contribution.status !== 'passed' && contribution.reason === undefined
        ? [{path: ['reason'], message: `a dev-qa contribution with status ${contribution.status} needs a reason`}]
        : []
    case 'dev-qa-verify':
      return repeatedVerdicts(contribution.verdicts.map(verdict => verdict.id))
    case 'code-trace':
      return [
        ...repeatedVerdicts(contribution.verdicts.map(verdict => verdict.id)),
        ...contribution.verdicts.flatMap((verdict, index) =>
          verdict.duplicateOf !== undefined && verdict.result === 'inconclusive'
            ? [{path: ['verdicts', index, 'duplicateOf'], message: 'duplicateOf is allowed only with root-cause or no-code-cause'}]
            : []
        )
      ]
    default: {
      const unreachable: never = contribution
      return unreachable
    }
  }
}

function crossFieldProblems(contribution: Contribution): Problem[] {
  const findings = contribution.kind === 'code-review' || contribution.kind === 'dev-qa' ? findingProblems(contribution) : []
  return [...kindProblems(contribution), ...findings, ...evidenceProblems(contribution)]
}

export const ContributionSchema = z
  .discriminatedUnion('kind', [CodeReviewSchema, DevQaSchema, DevQaVerifySchema, CodeTraceSchema])
  .superRefine((contribution, context) => {
    crossFieldProblems(contribution).forEach(problem => context.addIssue({code: z.ZodIssueCode.custom, ...problem}))
  })

function readJson(text: string): Parsed<unknown> {
  try {
    return {ok: true, value: JSON.parse(text)}
  } catch {
    return {ok: false, reason: 'not JSON'}
  }
}

export function checkContribution(value: unknown): Parsed<Contribution> {
  const parsed = ContributionSchema.safeParse(value)
  if (parsed.success) return {ok: true, value: parsed.data}
  const issue = parsed.error.issues[0]
  if (issue === undefined) return {ok: false, reason: 'invalid contribution'}
  const where = issue.path.length === 0 ? '(top level)' : issue.path.join('.')
  return {ok: false, reason: `${where}: ${issue.message}`}
}

export function parseContribution(text: string): Parsed<Contribution> {
  const json = readJson(text)
  return json.ok ? checkContribution(json.value) : json
}

export type NotRunInput = {kind: 'code-review' | 'dev-qa'; pr: number; head: string; platform?: string; reason: string}

export function notRun(input: NotRunInput): CodeReviewContribution | DevQaContribution {
  const empty = {comments: [], findings: [], uploads: [], handback: []}
  switch (input.kind) {
    case 'code-review':
      return {version: 1, kind: 'code-review', pr: input.pr, head: input.head, status: 'not-run', reason: input.reason, body: `Code review: not run. ${input.reason}`, ...empty}
    case 'dev-qa': {
      const label = input.platform === undefined ? 'Dev QA' : `Dev QA (${input.platform})`
      return {
        version: 1,
        kind: 'dev-qa',
        pr: input.pr,
        head: input.head,
        ...(input.platform === undefined ? {} : {platform: input.platform}),
        status: 'not-run',
        reason: input.reason,
        body: `${label}: not run. ${input.reason}`,
        ...empty
      }
    }
    default: {
      const unreachable: never = input.kind
      return unreachable
    }
  }
}
