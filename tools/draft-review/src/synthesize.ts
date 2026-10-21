import type {
  CodeReviewContribution,
  CodeTraceContribution,
  Contribution,
  ContributionComment,
  DevQaContribution,
  DevQaVerifyContribution,
  Finding,
  TraceVerdict
} from './contribution'

export type InlineComment = {path: string; line: number; side: 'LEFT' | 'RIGHT'; startLine?: number; body: string}
export type ReviewPayload = {body: string; comments: InlineComment[]; uploads: string[]; handback: string[]}
export type Synthesis = {ok: true; payload: ReviewPayload} | {ok: false; reason: string}

const ZONE_HEADERS = '# Pre-Claude zone\n\n# Claude zone'
const PARAGRAPH = '\n\n'

const unique = (values: string[]) => [...new Set(values)]
const repeated = (values: string[]) => unique(values.filter((value, index) => values.indexOf(value) !== index))
const isDefined = <Value>(value: Value | undefined): value is Value => value !== undefined
const platformSuffix = (platforms: string[]) => (platforms.length === 0 ? '' : ` (${platforms.join(', ')})`)
const location = (anchor: {path: string; line: number}) => `\`${anchor.path}\` line ${anchor.line}`

const reviewsOf = (contributions: Contribution[]) => contributions.filter((item): item is CodeReviewContribution => item.kind === 'code-review')
const devQasOf = (contributions: Contribution[]) => contributions.filter((item): item is DevQaContribution => item.kind === 'dev-qa')
const verifiesOf = (contributions: Contribution[]) => contributions.filter((item): item is DevQaVerifyContribution => item.kind === 'dev-qa-verify')
const tracesOf = (contributions: Contribution[]) => contributions.filter((item): item is CodeTraceContribution => item.kind === 'code-trace')

const qualify = (id: string, platform: string | undefined) => (platform === undefined ? id : `${id}@${platform}`)
const qualifiedFindings = (devQa: DevQaContribution): Finding[] => devQa.findings.map(finding => ({...finding, id: qualify(finding.id, devQa.platform)}))

export function routeFindings(contributions: Contribution[]): {toVerify: Finding[]; toTrace: Finding[]} {
  return {
    toVerify: reviewsOf(contributions).flatMap(review => review.findings.filter(finding => finding.behavior !== undefined)),
    toTrace: devQasOf(contributions).flatMap(devQa => qualifiedFindings(devQa).filter(finding => finding.kind === 'behavior'))
  }
}

function sameValue(label: string, values: string[]): string | undefined {
  const distinct = unique(values)
  return distinct.length > 1 ? `contributions name different ${label}: ${distinct.join(' and ')}` : undefined
}

function platformCollision(label: string, platforms: (string | undefined)[], requirePlatform: boolean): string | undefined {
  if (platforms.length < 2) return undefined
  const missing = platforms.filter(platform => platform === undefined).length
  if (requirePlatform && missing > 0) return `${platforms.length} ${label} contributions need distinct platforms, and ${missing} of them have no platform`
  const names = platforms.map(platform => platform ?? 'no platform')
  const shared = repeated(names)[0]
  return shared === undefined ? undefined : `two ${label} contributions share ${shared === 'no platform' ? 'no platform' : `the platform ${shared}`}`
}

function unknownIds(contributions: Contribution[]): string | undefined {
  const reviewIds = new Set(reviewsOf(contributions).flatMap(review => review.findings.map(finding => finding.id)))
  const devQaIds = new Set(devQasOf(contributions).flatMap(devQa => qualifiedFindings(devQa).map(finding => finding.id)))
  const verifyMiss = verifiesOf(contributions)
    .flatMap(verify => verify.verdicts)
    .find(verdict => !reviewIds.has(verdict.id))
  if (verifyMiss !== undefined) return `a dev-qa-verify verdict answers the unknown finding id ${verifyMiss.id}`
  const traceVerdicts = tracesOf(contributions).flatMap(traceItem => traceItem.verdicts)
  const traceMiss = traceVerdicts.find(verdict => !devQaIds.has(verdict.id))
  if (traceMiss !== undefined) return `a code-trace verdict answers the unknown finding id ${traceMiss.id}`
  const duplicateMiss = traceVerdicts.find(verdict => verdict.duplicateOf !== undefined && !reviewIds.has(verdict.duplicateOf))
  if (duplicateMiss?.duplicateOf !== undefined) return `the code-trace verdict for ${duplicateMiss.id} names the unknown review finding id ${duplicateMiss.duplicateOf}`
  return undefined
}

export function findConflict(contributions: Contribution[]): string | undefined {
  const checks: (() => string | undefined)[] = [
    () => (contributions.length === 0 ? 'no contributions to merge' : undefined),
    () => sameValue('pull requests', contributions.map(item => String(item.pr))),
    () => sameValue('heads', contributions.map(item => item.head)),
    () => (reviewsOf(contributions).length + devQasOf(contributions).length === 0 ? 'no code-review or dev-qa contribution to merge' : undefined),
    () => (reviewsOf(contributions).length > 1 ? `${reviewsOf(contributions).length} code-review contributions, expected at most one` : undefined),
    () => (tracesOf(contributions).length > 1 ? `${tracesOf(contributions).length} code-trace contributions, expected at most one` : undefined),
    () => platformCollision('Dev QA', devQasOf(contributions).map(item => item.platform), true),
    () => platformCollision('dev-qa-verify', verifiesOf(contributions).map(item => item.platform), false),
    () => unknownIds(contributions)
  ]
  return checks.map(check => check()).find(isDefined)
}

type Owned = {owner: DevQaContribution; finding: Finding}
type CrossCheck = {reviewNotes: Map<string, string[]>; reviewPointers: string[]; devQaPointers: Map<DevQaContribution, string[]>; traceComments: ContributionComment[]; handback: string[]}

const appendTo = <Key>(map: Map<Key, string[]>, key: Key, line: string) => new Map(map).set(key, [...(map.get(key) ?? []), line])

function reproducedNotes(contributions: Contribution[]): Pick<CrossCheck, 'reviewNotes' | 'reviewPointers'> {
  const verifies = verifiesOf(contributions)
  const reviewComments = reviewsOf(contributions).flatMap(review => review.comments)
  const reproduced = reviewsOf(contributions)
    .flatMap(review => review.findings)
    .map(finding => {
      const verdicts = verifies.flatMap(verify =>
        verify.verdicts.filter(verdict => verdict.id === finding.id && verdict.result === 'reproduced').map(verdict => ({verdict, platform: verify.platform}))
      )
      return {finding, verdicts}
    })
    .filter(({verdicts}) => verdicts.length > 0)
  const entries = reproduced.map(({finding, verdicts}) => {
    const heading = `Reproduced on the deployed build${platformSuffix(unique(verdicts.map(item => item.platform).filter(isDefined)))}`
    const evidence = unique(verdicts.flatMap(item => item.verdict.evidence))
    const comment = reviewComments.find(item => item.finding === finding.id)
    const pointer =
      comment === undefined
        ? [`${heading}: ${finding.title}.`, ...evidence].join(PARAGRAPH)
        : `${heading}: ${finding.title}. See the inline comment on ${location(comment)}.`
    return {id: finding.id, note: [`${heading}.`, ...evidence].join(PARAGRAPH), pointer}
  })
  return {
    reviewNotes: entries.reduce((notes, entry) => appendTo(notes, entry.id, entry.note), new Map<string, string[]>()),
    reviewPointers: entries.map(entry => entry.pointer)
  }
}

type TraceOutcome = {reviewNote?: {id: string; line: string}; pointer?: {owner: DevQaContribution; line: string}; comment?: ContributionComment}

function traceOutcome(verdict: TraceVerdict, owned: Owned, reviews: CodeReviewContribution[]): TraceOutcome {
  const {owner, finding} = owned
  if (verdict.result !== 'root-cause') return {}
  if (verdict.duplicateOf !== undefined) {
    const duplicateOf = verdict.duplicateOf
    const hasComment = reviews.some(review => review.comments.some(comment => comment.finding === duplicateOf))
    if (hasComment) {
      const line = [`Seen on the deployed build${platformSuffix([owner.platform].filter(isDefined))}: ${finding.title}.`, ...(finding.evidence ?? [])].join(PARAGRAPH)
      return {reviewNote: {id: duplicateOf, line}}
    }
    const reviewTitle = reviews.flatMap(review => review.findings).find(item => item.id === duplicateOf)?.title ?? duplicateOf
    return {pointer: {owner, line: `${finding.title}: same cause as the code finding "${reviewTitle}".`}}
  }
  if (verdict.anchor === undefined) return {pointer: {owner, line: [`${finding.title}: the cause is in the code.`, verdict.body].join(PARAGRAPH)}}
  return {
    comment: {...verdict.anchor, body: verdict.body, finding: verdict.id},
    pointer: {owner, line: `${finding.title}: the cause is in ${location(verdict.anchor)}. See the inline comment there.`}
  }
}

function traceNotes(contributions: Contribution[], start: Pick<CrossCheck, 'reviewNotes'>): Pick<CrossCheck, 'reviewNotes' | 'devQaPointers' | 'traceComments'> {
  const owned = new Map(devQasOf(contributions).flatMap(owner => qualifiedFindings(owner).map((finding): [string, Owned] => [finding.id, {owner, finding}])))
  const reviews = reviewsOf(contributions)
  const outcomes = tracesOf(contributions)
    .flatMap(traceItem => traceItem.verdicts)
    .map(verdict => {
      const target = owned.get(verdict.id)
      return target === undefined ? {} : traceOutcome(verdict, target, reviews)
    })
  return {
    reviewNotes: outcomes.reduce((notes, outcome) => (outcome.reviewNote === undefined ? notes : appendTo(notes, outcome.reviewNote.id, outcome.reviewNote.line)), start.reviewNotes),
    devQaPointers: outcomes.reduce(
      (pointers, outcome) => (outcome.pointer === undefined ? pointers : appendTo(pointers, outcome.pointer.owner, outcome.pointer.line)),
      new Map<DevQaContribution, string[]>()
    ),
    traceComments: outcomes.map(outcome => outcome.comment).filter(isDefined)
  }
}

function verdictHandback(contributions: Contribution[]): string[] {
  return verifiesOf(contributions).flatMap(verify =>
    verify.verdicts
      .filter(verdict => verdict.result !== 'reproduced')
      .map(verdict => `${verdict.id} ${verdict.result}${platformSuffix([verify.platform].filter(isDefined))}: ${verdict.note}`)
  )
}

function crossCheck(contributions: Contribution[]): CrossCheck {
  const reproduced = reproducedNotes(contributions)
  const traced = traceNotes(contributions, reproduced)
  return {
    reviewNotes: traced.reviewNotes,
    reviewPointers: reproduced.reviewPointers,
    devQaPointers: traced.devQaPointers,
    traceComments: traced.traceComments,
    handback: [...contributions.flatMap(item => item.handback), ...verdictHandback(contributions)]
  }
}

const notRunLine = (label: string, reason: string | undefined) => [`${label}: not run.`, reason].filter(isDefined).join(' ')

function reviewPart(review: CodeReviewContribution): string {
  return review.status === 'not-run' ? notRunLine('Code review', review.reason) : review.body
}

function devQaPart(devQa: DevQaContribution, pointers: string[], withHeading: boolean): string {
  const text = devQa.status === 'not-run' ? notRunLine(`Dev QA${platformSuffix([devQa.platform].filter(isDefined))}`, devQa.reason) : devQa.body
  const section = [text, ...pointers].join(PARAGRAPH)
  return withHeading && devQa.platform !== undefined ? `### ${devQa.platform}${PARAGRAPH}${section}` : section
}

function composeBody(contributions: Contribution[], check: CrossCheck, attribution: string | undefined): string {
  const devQas = devQasOf(contributions)
  return [
    ZONE_HEADERS,
    attribution,
    ...reviewsOf(contributions).map(reviewPart),
    ...devQas.map(devQa => devQaPart(devQa, check.devQaPointers.get(devQa) ?? [], devQas.length > 1)),
    ...check.reviewPointers
  ]
    .filter(isDefined)
    .join(PARAGRAPH)
}

function toInline(comment: ContributionComment, notes: string[], attribution: string | undefined): InlineComment {
  const body = [attribution, comment.body, ...notes].filter(isDefined).join(PARAGRAPH)
  return {path: comment.path, line: comment.line, side: comment.side, ...(comment.startLine === undefined ? {} : {startLine: comment.startLine}), body}
}

function composeComments(contributions: Contribution[], check: CrossCheck, attribution: string | undefined): InlineComment[] {
  const reviewComments = reviewsOf(contributions).flatMap(review => review.comments)
  const devQaComments = devQasOf(contributions)
    .filter(devQa => devQa.status !== 'not-run')
    .flatMap(devQa => devQa.comments)
  return [
    ...reviewComments.map(comment => toInline(comment, check.reviewNotes.get(comment.finding) ?? [], attribution)),
    ...[...devQaComments, ...check.traceComments].map(comment => toInline(comment, [], attribution))
  ]
}

function citedUploads(contributions: Contribution[], texts: string[]): string[] {
  const text = texts.join('\n')
  const candidates = unique(
    contributions.flatMap(item => [...item.uploads, ...(item.kind === 'dev-qa-verify' ? item.verdicts.flatMap(verdict => verdict.evidence) : [])])
  )
  return candidates
    .map(path => ({path, at: text.indexOf(path)}))
    .filter(({at}) => at >= 0)
    .sort((left, right) => left.at - right.at)
    .map(({path}) => path)
}

export function synthesize(contributions: Contribution[], attribution: string | undefined): Synthesis {
  const conflict = findConflict(contributions)
  if (conflict !== undefined) return {ok: false, reason: conflict}
  const check = crossCheck(contributions)
  const body = composeBody(contributions, check, attribution)
  const comments = composeComments(contributions, check, attribution)
  const uploads = citedUploads(contributions, [body, ...comments.map(comment => comment.body)])
  return {ok: true, payload: {body, comments, uploads, handback: check.handback}}
}
