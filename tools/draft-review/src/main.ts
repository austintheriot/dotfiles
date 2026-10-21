import type {AddCommand, CreateCommand, Repo, RouteCommand, ShowCommand} from './args'
import {parseCommand, USAGE} from './args'
import {DEFAULT_CONFIG_PATH, DEFAULT_STORE_ROOT, loadConfig, repositoryConfig, requireConfig} from './config'
import type {RepositoryConfig} from './config'
import type {ContributionSet} from './contribution-set'
import {routeFindings, validateContributions} from './contribution-set'
import type {DraftAction} from './draft'
import {bodyHash, draftAction} from './draft'
import {DraftReviewError, EXIT} from './exit'
import type {ExitCode} from './exit'
import {createPendingReview, currentRepo, deleteReview, ensureGh, ghUploader, pendingReviews, viewerLogin} from './github'
import {withLock} from './lock'
import type {ReviewPayload} from './payload'
import {renderReview} from './render'
import type {IUploader, PrStore} from './store'
import {prStore, readContributionFile, readContributions, readDraftRecord, resolveUploads, writeContribution, writeDraftRecord, writeRouted, writeSynthesized} from './store'
import {synthesize} from './synthesize'
import {substituteUploads} from './uploads'

const LOCK_WAIT_MS = 30_000

const storeFor = (command: {store: string | undefined}, repo: Repo, pr: number) => prStore(command.store ?? DEFAULT_STORE_ROOT, repo, pr)

function loadSet(store: PrStore, head: string): ContributionSet {
  const set = validateContributions(readContributions(store, head))
  if (!set.ok) throw new DraftReviewError(`the stored contributions for head ${head} cannot merge: ${set.reason}`, EXIT.dataErr)
  return set.value
}

async function add(command: AddCommand): Promise<ExitCode> {
  const contribution = readContributionFile(command.file)
  const store = storeFor(command, command.repo ?? currentRepo(), contribution.pr)
  const path = await withLock(store.lockPath, LOCK_WAIT_MS, async () => writeContribution(store, contribution))
  console.log(path)
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

type Posting = {repo: Repo; store: PrStore; command: CreateCommand; review: ReviewPayload}

function refuse(posting: Posting, reviewId: number | undefined): ExitCode {
  const path = writeSynthesized(posting.store, posting.command.head, renderReview(posting.review))
  const which = reviewId === undefined ? 'Another pending review' : `Pending review ${reviewId}`
  console.error(`${which} on pull request ${posting.command.pr} is not one that draft-review can replace. draft-review left it unchanged and wrote the review to a file.`)
  console.log(path)
  return EXIT.refused
}

function post(posting: Posting): ExitCode {
  const {repo, store, command, review} = posting
  const outcome = createPendingReview(repo, command.pr, command.head, review)
  if (outcome.kind === 'pending-exists') return refuse(posting, undefined)
  writeDraftRecord(store, {reviewId: outcome.review.id, bodyHash: bodyHash(outcome.review.body), head: command.head, url: outcome.review.url})
  console.log(outcome.review.url)
  return EXIT.ok
}

function applyDraftAction(action: DraftAction, posting: Posting): ExitCode {
  switch (action.kind) {
    case 'refuse':
      return refuse(posting, action.reviewId)
    case 'replace':
      deleteReview(posting.repo, posting.command.pr, action.reviewId)
      return post(posting)
    case 'create':
      return post(posting)
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
    const payload = synthesize(loadSet(store, command.head), settings.attribution)
    const urls = await resolveUploads(store, payload.uploads, firstPath => uploaderFor(settings, configPath, firstPath))
    const review = substituteUploads(payload, urls)
    ensureGh()
    const action = draftAction(pendingReviews(repo, command.pr, viewerLogin()), readDraftRecord(store))
    return applyDraftAction(action, {repo, store, command, review})
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
