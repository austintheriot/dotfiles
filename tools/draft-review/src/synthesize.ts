import type {CodeReviewContribution, ContributionComment, DevQaContribution, DevQaVerifyContribution, Finding, TraceVerdict, VerifyVerdict} from './contribution'
import type {ContributionSet} from './contribution-set'
import {qualifiedFindings} from './contribution-set'
import {isDefined, unique} from './lists'
import type {Parsed} from './parsed'
import {citedPaths} from './uploads'

export type InlineComment = {path: string; line: number; side: 'LEFT' | 'RIGHT'; startLine?: number; body: string}
export type ReviewPayload = {body: string; comments: InlineComment[]; uploads: string[]; handback: string[]}

const ZONE_HEADERS = '# Pre-Claude zone\n\n# Claude zone'
const PARAGRAPH = '\n\n'

const platformSuffix = (platforms: readonly (string | undefined)[]) => {
  const named = unique(platforms.filter(isDefined))
  return named.length === 0 ? '' : ` (${named.join(', ')})`
}
const location = (anchor: {path: string; line: number}) => `\`${anchor.path}\` line ${anchor.line}`
const reviewComments = (set: ContributionSet) => set.review?.comments ?? []
const reviewFindings = (set: ContributionSet) => set.review?.findings ?? []

type Part = DevQaContribution | 'before-dev-qa'
type Pointer = {part: Part; line: string}
type Notes = {reviewNotes: {id: string; line: string}[]; pointers: Pointer[]; traceComments: ContributionComment[]}

type Reproduction = {verdict: VerifyVerdict; platform: string | undefined; part: Part}

function reproductions(set: ContributionSet, finding: Finding): Reproduction[] {
  return set.verifies.flatMap((verify: DevQaVerifyContribution) =>
    verify.verdicts
      .filter(verdict => verdict.id === finding.id && verdict.result === 'reproduced')
      .map(verdict => ({verdict, platform: verify.platform, part: set.devQas.find(devQa => devQa.platform === verify.platform) ?? 'before-dev-qa'}))
  )
}

function reproducedNotes(set: ContributionSet): Notes {
  const entries = reviewFindings(set)
    .map(finding => ({finding, found: reproductions(set, finding)}))
    .filter(({found}) => found.length > 0)
  return {
    reviewNotes: entries.map(({finding, found}) => ({
      id: finding.id,
      line: [`Reproduced on the deployed build${platformSuffix(found.map(item => item.platform))}.`, ...unique(found.flatMap(item => item.verdict.evidence))].join(PARAGRAPH)
    })),
    pointers: entries.flatMap(({finding, found}) => {
      const comment = reviewComments(set).find(item => item.finding === finding.id)
      return unique(found.map(item => item.part)).map(part => {
        const here = found.filter(item => item.part === part)
        const heading = `Reproduced on the deployed build${platformSuffix(here.map(item => item.platform))}: ${finding.title}.`
        const line =
          comment === undefined
            ? [heading, ...unique(here.flatMap(item => item.verdict.evidence))].join(PARAGRAPH)
            : `${heading} See the inline comment on ${location(comment)}.`
        return {part, line}
      })
    }),
    traceComments: []
  }
}

function traceNote(set: ContributionSet, verdict: TraceVerdict, owner: DevQaContribution, finding: Finding): Notes {
  const none: Notes = {reviewNotes: [], pointers: [], traceComments: []}
  if (verdict.result !== 'root-cause') return none
  if (verdict.duplicateOf !== undefined) {
    const duplicateOf = verdict.duplicateOf
    if (reviewComments(set).some(comment => comment.finding === duplicateOf)) {
      const line = [`Seen on the deployed build${platformSuffix([owner.platform])}: ${finding.title}.`, ...(finding.evidence ?? [])].join(PARAGRAPH)
      return {...none, reviewNotes: [{id: duplicateOf, line}]}
    }
    const reviewTitle = reviewFindings(set).find(item => item.id === duplicateOf)?.title ?? duplicateOf
    return {...none, pointers: [{part: owner, line: `${finding.title}: same cause as the code finding "${reviewTitle}".`}]}
  }
  if (verdict.anchor === undefined) return {...none, pointers: [{part: owner, line: [`${finding.title}: the cause is in the code.`, verdict.body].join(PARAGRAPH)}]}
  return {
    reviewNotes: [],
    pointers: [{part: owner, line: `${finding.title}: the cause is in ${location(verdict.anchor)}. See the inline comment there.`}],
    traceComments: [{...verdict.anchor, body: verdict.body, finding: verdict.id}]
  }
}

function traceNotes(set: ContributionSet): Notes[] {
  const owned = new Map(set.devQas.flatMap(owner => qualifiedFindings(owner).map((finding): [string, {owner: DevQaContribution; finding: Finding}] => [finding.id, {owner, finding}])))
  return (set.trace?.verdicts ?? []).map(verdict => {
    const target = owned.get(verdict.id)
    return target === undefined ? {reviewNotes: [], pointers: [], traceComments: []} : traceNote(set, verdict, target.owner, target.finding)
  })
}

function crossCheck(set: ContributionSet): Notes {
  const all = [...traceNotes(set), reproducedNotes(set)]
  return {
    reviewNotes: all.flatMap(notes => notes.reviewNotes),
    pointers: all.flatMap(notes => notes.pointers),
    traceComments: all.flatMap(notes => notes.traceComments)
  }
}

function handback(set: ContributionSet): string[] {
  const lines = [set.review, ...set.devQas, ...set.verifies, set.trace].filter(isDefined).flatMap(item => item.handback)
  const verdictLines = set.verifies.flatMap(verify =>
    verify.verdicts
      .filter(verdict => verdict.result !== 'reproduced')
      .map(verdict => `${verdict.id} ${verdict.result}${platformSuffix([verify.platform])}: ${verdict.note}`)
  )
  return [...lines, ...verdictLines]
}

const notRunLine = (label: string, reason: string | undefined) => [`${label}: not run.`, reason].filter(isDefined).join(' ')

const reviewPart = (review: CodeReviewContribution) => (review.status === 'not-run' ? notRunLine('Code review', review.reason) : review.body)

const pointersFor = (notes: Notes, part: Part) => notes.pointers.filter(pointer => pointer.part === part).map(pointer => pointer.line)

function devQaPart(devQa: DevQaContribution, pointers: string[], withHeading: boolean): string {
  const text = devQa.status === 'not-run' ? notRunLine(`Dev QA${platformSuffix([devQa.platform])}`, devQa.reason) : devQa.body
  const section = [text, ...pointers].join(PARAGRAPH)
  return withHeading && devQa.platform !== undefined ? `### ${devQa.platform}${PARAGRAPH}${section}` : section
}

function composeBody(set: ContributionSet, notes: Notes, attribution: string | undefined): string {
  return [
    ZONE_HEADERS,
    attribution,
    set.review === undefined ? undefined : reviewPart(set.review),
    ...pointersFor(notes, 'before-dev-qa'),
    ...set.devQas.map(devQa => devQaPart(devQa, pointersFor(notes, devQa), set.devQas.length > 1))
  ]
    .filter(isDefined)
    .join(PARAGRAPH)
}

function toInline(comment: ContributionComment, notes: string[], attribution: string | undefined): InlineComment {
  const body = [attribution, comment.body, ...notes].filter(isDefined).join(PARAGRAPH)
  return {path: comment.path, line: comment.line, side: comment.side, ...(comment.startLine === undefined ? {} : {startLine: comment.startLine}), body}
}

function composeComments(set: ContributionSet, notes: Notes, attribution: string | undefined): InlineComment[] {
  const notesFor = (id: string) => notes.reviewNotes.filter(note => note.id === id).map(note => note.line)
  const devQaComments = set.devQas.flatMap(devQa => devQa.comments)
  return [
    ...reviewComments(set).map(comment => toInline(comment, notesFor(comment.finding), attribution)),
    ...[...devQaComments, ...notes.traceComments].map(comment => toInline(comment, [], attribution))
  ]
}

function candidateUploads(set: ContributionSet): string[] {
  return [set.review, ...set.devQas, ...set.verifies, set.trace]
    .filter(isDefined)
    .flatMap(item => [...item.uploads, ...(item.kind === 'dev-qa-verify' ? item.verdicts.flatMap(verdict => verdict.evidence) : [])])
}

export function synthesize(set: ContributionSet, attribution: string | undefined): Parsed<ReviewPayload> {
  const signature = attribution === undefined || attribution.trim() === '' ? undefined : attribution
  const notes = crossCheck(set)
  const body = composeBody(set, notes, signature)
  const comments = composeComments(set, notes, signature)
  const uploads = citedPaths([body, ...comments.map(comment => comment.body)], candidateUploads(set))
  return {ok: true, value: {body, comments, uploads, handback: handback(set)}}
}
