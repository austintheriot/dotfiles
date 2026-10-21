import type {AddCommand, CreateCommand, Repo, RouteCommand, ShowCommand} from './args'
import {parseCommand, USAGE} from './args'
import {DEFAULT_CONFIG_PATH, DEFAULT_STORE_ROOT, loadConfig, repositoryConfig, requireConfig} from './config'
import type {RepositoryConfig} from './config'
import type {ContributionSet} from './contribution-set'
import {routeFindings, validateContributions} from './contribution-set'
import type {Contribution} from './contribution'
import type {DraftAction, ReadBack} from './draft'
import {draftAction, draftRecordAfterCreate} from './draft'
import {DraftReviewError, EXIT} from './exit'
import type {ExitCode} from './exit'
import type {CreatedReview} from './github'
import {createPendingReview, currentRepo, deleteReview, ensureGh, ghUploader, recordedReview, reviewComments, reviewRequest, viewerLogin, viewerReviews} from './github'
import {withLock} from './lock'
import type {ReviewPayload} from './payload'
import {renderReview} from './render'
import type {IUploader, PrStore} from './store'
import {
  headDigest,
  prStore,
  readHead,
  readContributionFile,
  readContributions,
  readDraftRecord,
  prepareUploads,
  runUploads,
  writeContribution,
  writeDraftRecord,
  writeReviewRequest,
  writeRouted,
  writeSynthesized
} from './store'
import {synthesize} from './synthesize'
import {substituteUploads} from './uploads'

const LOCK_WAIT_MS = 30_000

const storeFor = (command: {store: string | undefined}, repo: Repo, pr: number) => prStore(command.store ?? DEFAULT_STORE_ROOT, repo, pr)

function mergeable(contributions: Contribution[], head: string): ContributionSet {
  const set = validateContributions(contributions)
  if (!set.ok) throw new DraftReviewError(`the stored contributions for head ${head} cannot merge: ${set.reason}`, EXIT.dataErr)
  return set.value
}

const loadSet = (store: PrStore, head: string): ContributionSet => mergeable(readContributions(store, head), head)

async function add(command: AddCommand): Promise<ExitCode> {
  const contribution = readContributionFile(command.file)
  const store = storeFor(command, command.repo ?? currentRepo(), contribution.pr)
  console.log(writeContribution(store, contribution))
  return EXIT.ok
}

function route(command: RouteCommand): ExitCode {
  const store = storeFor(command, command.repo ?? currentRepo(), command.pr)
  writeRouted(command.out, routeFindings(loadSet(store, command.head))).forEach(path => console.log(path))
  return EXIT.ok
}

function showAttribution(configPath: string, repo: Repo): string | undefined {
  const config = loadConfig(configPath)
  const entry = config?.repositories[`${repo.owner}/${repo.name}`]
  if (entry === undefined) console.error(`No entry for ${repo.owner}/${repo.name} in ${configPath}, so this shows no attribution line, and create will refuse.`)
  return entry?.attribution
}

function show(command: ShowCommand): ExitCode {
  const repo = command.repo ?? currentRepo()
  const attribution = showAttribution(command.config ?? DEFAULT_CONFIG_PATH, repo)
  const store = storeFor(command, repo, command.pr)
  process.stdout.write(renderReview(synthesize(loadSet(store, command.head), attribution)))
  return EXIT.ok
}

function uploaderFor(settings: RepositoryConfig, configPath: string, firstPath: string): IUploader {
  if (settings.uploadEndpoint === undefined) {
    throw new DraftReviewError(`the review cites ${firstPath}, which needs an upload, and ${configPath} sets no uploadEndpoint for this repository`, EXIT.config)
  }
  ensureGh()
  return ghUploader(settings.uploadEndpoint, settings.repositoryId)
}

type Target = {repo: Repo; store: PrStore; command: CreateCommand; login: string; contributionsDigest: string}
type Written = {synthesizedPath: string; requestPath: string; requestedBodies: string[]; body: string}

const decide = (target: Target, body: string): DraftAction => {
  const {repo, command} = target
  const reviews = viewerReviews(repo, command.pr, target.login)
  const recorded = recordedReview(repo, command.pr, readDraftRecord(target.store))
  return draftAction({pending: reviews.pending, submitted: reviews.submitted, recorded, head: command.head, body})
}

function refuseDuplicate(target: Target, synthesizedPath: string, action: {reviewId: number; state: string}): ExitCode {
  const {head} = target.command
  console.error(
    `A submitted review for head ${head} (review ${action.reviewId}, state ${action.state}) already holds this content. draft-review never posts submitted content again. The merged review is in ${synthesizedPath}.`
  )
  console.log(synthesizedPath)
  return EXIT.refused
}

type Refusal = Extract<DraftAction, {kind: 'refuse' | 'submitted' | 'duplicate'}>

const isRefusal = (action: DraftAction): action is Refusal => action.kind === 'refuse' || action.kind === 'submitted' || action.kind === 'duplicate'

function refuseFor(action: Refusal, target: Target, synthesizedPath: string): ExitCode {
  switch (action.kind) {
    case 'refuse':
      return refuse(target, synthesizedPath, action.reviewId)
    case 'submitted':
      return refuseSubmitted(target, synthesizedPath, action)
    case 'duplicate':
      return refuseDuplicate(target, synthesizedPath, action)
    default: {
      const unreachable: never = action
      return unreachable
    }
  }
}

function refuseSubmitted(target: Target, synthesizedPath: string, action: {reviewId: number; state: string}): ExitCode {
  const {head} = target.command
  console.error(
    `The first draft for head ${head} (review ${action.reviewId}) was submitted, and its state is ${action.state}. draft-review never posts submitted content again. The merged review is in ${synthesizedPath}.`
  )
  console.log(synthesizedPath)
  return EXIT.refused
}

function refuse(target: Target, synthesizedPath: string, reviewId: number | undefined): ExitCode {
  const which = reviewId === undefined ? 'Another pending review' : `Pending review ${reviewId}`
  console.error(`${which} on pull request ${target.command.pr} is not one that draft-review can replace. draft-review left it unchanged and wrote the review to a file.`)
  console.log(synthesizedPath)
  return EXIT.refused
}

function writeReview(target: Target, review: ReviewPayload): Written {
  const {store, command} = target
  return {
    synthesizedPath: writeSynthesized(store, command.head, renderReview(review)),
    requestPath: writeReviewRequest(store, command.head, reviewRequest(command.head, review)),
    requestedBodies: review.comments.map(comment => comment.body),
    body: review.body
  }
}

function readBack(target: Target, reviewId: number): ReadBack {
  try {
    return {kind: 'read', comments: reviewComments(target.repo, target.command.pr, reviewId)}
  } catch (error) {
    if (!(error instanceof DraftReviewError)) throw error
    return {kind: 'failed', reason: error.message}
  }
}

const SKIP_EXIT = {'read-failed': EXIT.unavailable, mismatch: EXIT.refused} as const

function recordDraft(target: Target, written: Written, review: CreatedReview): void {
  const {store, command, contributionsDigest} = target
  const decision = draftRecordAfterCreate({review, head: command.head, requestedBodies: written.requestedBodies, readBack: readBack(target, review.id), contributionsDigest})
  if (decision.kind === 'record') return writeDraftRecord(store, decision.record)
  const consequence =
    'draft-review wrote no draft.json, so the next create refuses to replace that review while it is pending. When you submit it, create never posts the same content again. Check the review on GitHub, delete it, then run create again.'
  throw new DraftReviewError(`pending review ${review.id} exists, but ${decision.reason}. ${consequence}`, SKIP_EXIT[decision.cause])
}

function refuseStale(target: Target, reviewId: number): void {
  const {store, command} = target
  if (headDigest(store, command.head) === target.contributionsDigest) return
  const message = `a contribution for head ${command.head} changed while create ran, so pending review ${reviewId} is stale. Run create again.`
  throw new DraftReviewError(message, EXIT.tempFail)
}

function lostDraft(written: Written, deletedId: number, cause: string): DraftReviewError {
  const steps = `The review request is in ${written.requestPath}. Run create again to make the draft again.`
  return new DraftReviewError(`draft-review deleted pending review ${deletedId}, and then the new draft was not made: ${cause}. ${steps}`, EXIT.unavailable)
}

function post(target: Target, written: Written, deletedId: number | undefined): ExitCode {
  const outcome = (() => {
    try {
      return createPendingReview(target.repo, target.command.pr, written.requestPath)
    } catch (error) {
      if (deletedId === undefined || !(error instanceof DraftReviewError)) throw error
      throw lostDraft(written, deletedId, error.message)
    }
  })()
  if (outcome.kind === 'pending-exists') {
    if (deletedId !== undefined) throw lostDraft(written, deletedId, 'another pending review appeared on the pull request')
    return refuse(target, written.synthesizedPath, undefined)
  }
  console.log(outcome.review.url)
  recordDraft(target, written, outcome.review)
  refuseStale(target, outcome.review.id)
  return EXIT.ok
}

function applyDraftAction(action: DraftAction, target: Target, written: Written): ExitCode {
  if (isRefusal(action)) return refuseFor(action, target, written.synthesizedPath)
  switch (action.kind) {
    case 'replace':
      deleteReview(target.repo, target.command.pr, action.reviewId)
      return post(target, written, action.reviewId)
    case 'create':
      return post(target, written, undefined)
    default: {
      const unreachable: never = action
      return unreachable
    }
  }
}

async function create(command: CreateCommand): Promise<ExitCode> {
  const configPath = command.config ?? DEFAULT_CONFIG_PATH
  const config = requireConfig(configPath)
  const repo = command.repo ?? currentRepo()
  const settings = repositoryConfig(config, configPath, repo)
  const store = storeFor(command, repo, command.pr)
  return withLock(store.lockPath, LOCK_WAIT_MS, async () => {
    const {contributions, digest} = readHead(store, command.head)
    const payload = synthesize(mergeable(contributions, command.head), settings.attribution)
    const uploads = prepareUploads(store, command.head, payload.uploads, firstPath => uploaderFor(settings, configPath, firstPath))
    ensureGh()
    const target: Target = {repo, store, command, login: viewerLogin(), contributionsDigest: digest}
    const early = decide(target, substituteUploads(payload, uploads.plan.urlByPath).body)
    if (isRefusal(early)) return refuseFor(early, target, writeSynthesized(store, command.head, renderReview(payload)))
    const urls = await runUploads(store, uploads)
    const written = writeReview(target, substituteUploads(payload, urls))
    return applyDraftAction(decide(target, written.body), target, written)
  })
}

async function main(argv: readonly string[]): Promise<number> {
  const parsed = parseCommand(argv)
  if (!parsed.ok) {
    console.error(`draft-review: ${parsed.reason}\n\n${USAGE}`)
    return EXIT.usage
  }
  const command = parsed.value
  try {
    switch (command.kind) {
      case 'help':
        console.log(USAGE)
        return EXIT.ok
      case 'add':
        return await add(command)
      case 'route':
        return route(command)
      case 'show':
        return show(command)
      case 'create':
        return await create(command)
      default: {
        const unreachable: never = command
        return unreachable
      }
    }
  } catch (error) {
    if (!(error instanceof DraftReviewError)) throw error
    console.error(`draft-review: ${error.message}`)
    return error.exitCode
  }
}

process.exit(await main(process.argv.slice(2)))
