import {z} from 'zod'
import type {Parsed} from './parsed'

const Text = z.string().min(1)
const Line = z.number().int().positive()
const Side = z.enum(['LEFT', 'RIGHT'])
const LocalPaths = z.array(Text)

const Anchor = z.object({path: Text, line: Line, side: Side}).strict()

const ContributionComment = z
  .object({path: Text, line: Line, side: Side, startLine: Line.optional(), body: Text, finding: Text})
  .strict()

const FindingSchema = z
  .object({
    id: Text,
    kind: z.enum(['code', 'behavior']),
    title: Text,
    body: Text,
    anchor: Anchor.optional(),
    behavior: Text.optional(),
    evidence: LocalPaths.optional()
  })
  .strict()

const VerifyVerdictSchema = z
  .object({id: Text, result: z.enum(['reproduced', 'not-reproduced', 'not-reachable']), evidence: LocalPaths, note: Text})
  .strict()

const TraceVerdictSchema = z
  .object({
    id: Text,
    result: z.enum(['root-cause', 'no-code-cause', 'inconclusive']),
    anchor: Anchor.optional(),
    body: Text,
    duplicateOf: Text.optional()
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
  comments: z.array(ContributionComment),
  findings: z.array(FindingSchema)
}

const CodeReviewSchema = z
  .object({...common, ...roundOne, kind: z.literal('code-review'), status: z.enum(['completed', 'not-run']).optional(), reason: Text.optional()})
  .strict()

const DevQaSchema = z
  .object({
    ...common,
    ...roundOne,
    kind: z.literal('dev-qa'),
    platform: Text.optional(),
    status: z.enum(['passed', 'failed', 'incomplete', 'not-run']),
    reason: Text.optional()
  })
  .strict()

const DevQaVerifySchema = z
  .object({...common, kind: z.literal('dev-qa-verify'), platform: Text.optional(), verdicts: z.array(VerifyVerdictSchema)})
  .strict()

const CodeTraceSchema = z.object({...common, kind: z.literal('code-trace'), verdicts: z.array(TraceVerdictSchema)}).strict()

export type Anchor = z.infer<typeof Anchor>
export type ContributionComment = z.infer<typeof ContributionComment>
export type Finding = z.infer<typeof FindingSchema>
export type VerifyVerdict = z.infer<typeof VerifyVerdictSchema>
export type TraceVerdict = z.infer<typeof TraceVerdictSchema>
export type CodeReviewContribution = z.infer<typeof CodeReviewSchema>
export type DevQaContribution = z.infer<typeof DevQaSchema>
export type DevQaVerifyContribution = z.infer<typeof DevQaVerifySchema>
export type CodeTraceContribution = z.infer<typeof CodeTraceSchema>
export type Contribution = CodeReviewContribution | DevQaContribution | DevQaVerifyContribution | CodeTraceContribution

type Problem = {path: (string | number)[]; message: string}

const duplicates = (values: string[]) => [...new Set(values.filter((value, index) => values.indexOf(value) !== index))]

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

function findingProblems(findings: Finding[], comments: ContributionComment[]): Problem[] {
  const ids = new Set(findings.map(finding => finding.id))
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
  return [...repeated, ...behaviorGaps, ...commentGaps]
}

function kindProblems(contribution: Contribution): Problem[] {
  switch (contribution.kind) {
    case 'code-review': {
      if (contribution.status !== 'not-run') return []
      return [
        ...(contribution.reason === undefined ? [{path: ['reason'], message: 'a not-run code-review needs a reason'}] : []),
        ...(contribution.findings.length > 0 ? [{path: ['findings'], message: 'a not-run code-review has no findings'}] : []),
        ...(contribution.comments.length > 0 ? [{path: ['comments'], message: 'a not-run code-review has no comments'}] : [])
      ]
    }
    case 'dev-qa':
      return contribution.status !== 'passed' && contribution.reason === undefined
        ? [{path: ['reason'], message: `a dev-qa contribution with status ${contribution.status} needs a reason`}]
        : []
    case 'dev-qa-verify':
      return duplicates(contribution.verdicts.map(verdict => verdict.id)).map(id => ({path: ['verdicts'], message: `verdict id ${id} appears more than once`}))
    case 'code-trace':
      return [
        ...duplicates(contribution.verdicts.map(verdict => verdict.id)).map(id => ({path: ['verdicts'], message: `verdict id ${id} appears more than once`})),
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
  const findings = contribution.kind === 'code-review' || contribution.kind === 'dev-qa' ? findingProblems(contribution.findings, contribution.comments) : []
  return [...kindProblems(contribution), ...findings, ...evidenceProblems(contribution)]
}

const ContributionSchema = z
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

export function parseContribution(text: string): Parsed<Contribution> {
  const json = readJson(text)
  if (!json.ok) return json
  const parsed = ContributionSchema.safeParse(json.value)
  if (parsed.success) return {ok: true, value: parsed.data}
  const issue = parsed.error.issues[0]
  if (issue === undefined) return {ok: false, reason: 'invalid contribution'}
  const where = issue.path.length === 0 ? '(top level)' : issue.path.join('.')
  return {ok: false, reason: `${where}: ${issue.message}`}
}

export type NotRunInput = {kind: 'code-review' | 'dev-qa'; pr: number; head: string; platform?: string; reason: string}

export function notRun(input: NotRunInput): Contribution {
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
