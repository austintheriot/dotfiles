import {checkContribution} from './contribution'
import type {CodeReviewContribution, CodeTraceContribution, Contribution, DevQaContribution, DevQaVerifyContribution, Finding} from './contribution'
import {duplicates, isDefined, unique} from './lists'
import type {Parsed} from './parsed'

const constructionToken: unique symbol = Symbol('contribution set construction token')

type Members = {
  review: CodeReviewContribution | undefined
  devQas: readonly DevQaContribution[]
  verifies: readonly DevQaVerifyContribution[]
  trace: CodeTraceContribution | undefined
}

export class ContributionSet {
  readonly #members: Members

  public constructor(token: typeof constructionToken, members: Members) {
    if (token !== constructionToken) throw new Error('only validateContributions can build a ContributionSet')
    this.#members = members
  }

  public get review(): CodeReviewContribution | undefined {
    return this.#members.review
  }

  public get devQas(): readonly DevQaContribution[] {
    return this.#members.devQas
  }

  public get verifies(): readonly DevQaVerifyContribution[] {
    return this.#members.verifies
  }

  public get trace(): CodeTraceContribution | undefined {
    return this.#members.trace
  }
}

export type RoutedFindings = {toVerify: Finding[]; toTrace: Finding[]}

const qualify = (id: string, platform: string | undefined) => (platform === undefined ? id : `${id}@${platform}`)

export const qualifiedFindings = (devQa: DevQaContribution): Finding[] => devQa.findings.map(finding => ({...finding, id: qualify(finding.id, devQa.platform)}))

function route(reviews: readonly CodeReviewContribution[], devQas: readonly DevQaContribution[]): RoutedFindings {
  return {
    toVerify: reviews.flatMap(review => review.findings.filter(finding => finding.behavior !== undefined)),
    toTrace: devQas.flatMap(devQa => qualifiedFindings(devQa).filter(finding => finding.kind === 'behavior'))
  }
}

export function routeFindings(set: ContributionSet): RoutedFindings {
  return route([set.review].filter(isDefined), set.devQas)
}

type Grouped = {
  reviews: CodeReviewContribution[]
  devQas: DevQaContribution[]
  verifies: DevQaVerifyContribution[]
  traces: CodeTraceContribution[]
}

function group(contributions: readonly Contribution[]): Grouped {
  return {
    reviews: contributions.filter((item): item is CodeReviewContribution => item.kind === 'code-review'),
    devQas: contributions.filter((item): item is DevQaContribution => item.kind === 'dev-qa'),
    verifies: contributions.filter((item): item is DevQaVerifyContribution => item.kind === 'dev-qa-verify'),
    traces: contributions.filter((item): item is CodeTraceContribution => item.kind === 'code-trace')
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
  const shared = duplicates(platforms.map(platform => platform ?? 'no platform'))[0]
  return shared === undefined ? undefined : `two ${label} contributions share ${shared === 'no platform' ? 'no platform' : `the platform ${shared}`}`
}

function unroutedVerdict(grouped: Grouped): string | undefined {
  const routed = route(grouped.reviews, grouped.devQas)
  const verifyIds = new Set(routed.toVerify.map(finding => finding.id))
  const traceIds = new Set(routed.toTrace.map(finding => finding.id))
  const reviewIds = new Set(grouped.reviews.flatMap(review => review.findings.map(finding => finding.id)))
  const verifyMiss = grouped.verifies.flatMap(verify => verify.verdicts).find(verdict => !verifyIds.has(verdict.id))
  if (verifyMiss !== undefined) return `a dev-qa-verify verdict answers ${verifyMiss.id}, which is not a review finding with a behavior`
  const traceVerdicts = grouped.traces.flatMap(trace => trace.verdicts)
  const traceMiss = traceVerdicts.find(verdict => !traceIds.has(verdict.id))
  if (traceMiss !== undefined) return `a code-trace verdict answers ${traceMiss.id}, which is not a Dev QA behavior finding`
  const duplicateMiss = traceVerdicts.find(verdict => verdict.duplicateOf !== undefined && !reviewIds.has(verdict.duplicateOf))
  if (duplicateMiss?.duplicateOf !== undefined) return `the code-trace verdict for ${duplicateMiss.id} names the unknown review finding id ${duplicateMiss.duplicateOf}`
  return undefined
}

function findConflict(contributions: readonly Contribution[], grouped: Grouped): string | undefined {
  const checks: (() => string | undefined)[] = [
    () => sameValue('pull requests', contributions.map(item => String(item.pr))),
    () => sameValue('heads', contributions.map(item => item.head)),
    () => (grouped.reviews.length + grouped.devQas.length === 0 ? 'no code-review or dev-qa contribution to merge' : undefined),
    () => (grouped.reviews.length > 1 ? `${grouped.reviews.length} code-review contributions, expected at most one` : undefined),
    () => (grouped.traces.length > 1 ? `${grouped.traces.length} code-trace contributions, expected at most one` : undefined),
    () => platformCollision('Dev QA', grouped.devQas.map(item => item.platform), true),
    () => platformCollision('dev-qa-verify', grouped.verifies.map(item => item.platform), false),
    () => unroutedVerdict(grouped)
  ]
  return checks.map(check => check()).find(isDefined)
}

export function validateContributions(list: readonly Contribution[]): Parsed<ContributionSet> {
  if (list.length === 0) return {ok: false, reason: 'no contributions to merge'}
  const checked = list.map(checkContribution)
  const failure = checked.map((result, index) => (result.ok ? undefined : `contribution ${index + 1}: ${result.reason}`)).find(isDefined)
  if (failure !== undefined) return {ok: false, reason: failure}
  const contributions = checked.flatMap(result => (result.ok ? [result.value] : []))
  const grouped = group(contributions)
  const conflict = findConflict(contributions, grouped)
  if (conflict !== undefined) return {ok: false, reason: conflict}
  const [review] = grouped.reviews
  const [trace] = grouped.traces
  return {ok: true, value: new ContributionSet(constructionToken, {review, devQas: grouped.devQas, verifies: grouped.verifies, trace})}
}
