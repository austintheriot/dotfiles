import {basename, extname} from 'node:path'
import {z} from 'zod'
import type {Repo} from './args'
import {parseRepo} from './args'
import type {PendingReview} from './draft'
import {DraftReviewError, EXIT} from './exit'
import type {ReviewPayload} from './payload'
import type {IUploader} from './store'

type GhRun = {code: number; stdout: string; stderr: string}

function gh(args: readonly string[], input?: string): GhRun {
  try {
    const run = Bun.spawnSync(['gh', ...args], {stdin: input === undefined ? 'ignore' : Buffer.from(input), stdout: 'pipe', stderr: 'pipe'})
    return {code: run.exitCode, stdout: run.stdout.toString(), stderr: run.stderr.toString()}
  } catch (error) {
    throw new DraftReviewError(`cannot run gh: ${error instanceof Error ? error.message : String(error)}. Install the GitHub CLI.`, EXIT.unavailable)
  }
}

function readJson(text: string): unknown {
  try {
    return JSON.parse(text)
  } catch {
    return undefined
  }
}

function parseReply<Schema extends z.ZodTypeAny>(run: GhRun, schema: Schema, what: string): z.infer<Schema> {
  if (run.code !== 0) throw new DraftReviewError(`gh could not ${what}: ${[run.stderr.trim(), run.stdout.trim()].filter(text => text !== '').join(' ')}`, EXIT.unavailable)
  const parsed = schema.safeParse(readJson(run.stdout))
  if (!parsed.success) throw new DraftReviewError(`gh gave an unexpected reply when asked to ${what}: ${run.stdout.slice(0, 200)}`, EXIT.unavailable)
  return parsed.data
}

const ghJson = <Schema extends z.ZodTypeAny>(args: readonly string[], schema: Schema, what: string): z.infer<Schema> => parseReply(gh(args), schema, what)

const reviewsPath = (repo: Repo, pr: number) => `repos/${repo.owner}/${repo.name}/pulls/${pr}/reviews`

export function ensureGh(): void {
  const run = gh(['auth', 'status'])
  if (run.code !== 0) throw new DraftReviewError(`gh is not signed in: ${run.stderr.trim()}. Run gh auth login.`, EXIT.unavailable)
}

export function currentRepo(): Repo {
  const view = ghJson(['repo', 'view', '--json', 'nameWithOwner'], z.object({nameWithOwner: z.string()}), 'find the repository of the current directory. Pass --repo <owner/repo>')
  const repo = parseRepo(view.nameWithOwner)
  if (!repo.ok) throw new DraftReviewError(repo.reason, EXIT.unavailable)
  return repo.value
}

export const viewerLogin = (): string => ghJson(['api', 'user'], z.object({login: z.string()}), 'read the signed-in user').login

const ReviewSchema = z.object({id: z.number().int(), state: z.string(), body: z.string().nullable(), user: z.object({login: z.string()}).nullable()})

export function pendingReviews(repo: Repo, pr: number, login: string): PendingReview[] {
  const pages = ghJson(['api', reviewsPath(repo, pr), '--paginate', '--slurp'], z.array(z.array(ReviewSchema)), `list the reviews of pull request ${pr}`)
  return pages
    .flat()
    .filter(review => review.state === 'PENDING' && review.user?.login === login)
    .map(review => ({id: review.id, body: review.body ?? ''}))
}

export function deleteReview(repo: Repo, pr: number, reviewId: number): void {
  const run = gh(['api', '--method', 'DELETE', `${reviewsPath(repo, pr)}/${reviewId}`])
  if (run.code !== 0) throw new DraftReviewError(`gh could not delete pending review ${reviewId}: ${run.stderr.trim()}`, EXIT.unavailable)
}

export type CreatedReview = {id: number; url: string; body: string}
export type CreateOutcome = {kind: 'created'; review: CreatedReview} | {kind: 'pending-exists'}

function reviewRequest(head: string, payload: ReviewPayload) {
  const comments = payload.comments.map(comment => ({
    path: comment.path,
    line: comment.line,
    side: comment.side,
    ...(comment.startLine === undefined || comment.startLine === comment.line ? {} : {start_line: comment.startLine, start_side: comment.side}),
    body: comment.body
  }))
  return {commit_id: head, body: payload.body, ...(comments.length === 0 ? {} : {comments})}
}

// Omitting `event` is what keeps the review pending. GitHub submits a review only when the request names an event.
export function createPendingReview(repo: Repo, pr: number, head: string, payload: ReviewPayload): CreateOutcome {
  const run = gh(['api', '--method', 'POST', reviewsPath(repo, pr), '--input', '-'], JSON.stringify(reviewRequest(head, payload)))
  if (run.code !== 0 && /one pending review/i.test(run.stdout + run.stderr)) return {kind: 'pending-exists'}
  const created = parseReply(run, z.object({id: z.number().int(), html_url: z.string(), body: z.string().nullable()}), 'create the pending review')
  return {kind: 'created', review: {id: created.id, url: created.html_url, body: created.body ?? ''}}
}

const CONTENT_TYPES: Record<string, string> = {
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.jpeg': 'image/jpeg',
  '.gif': 'image/gif',
  '.webp': 'image/webp',
  '.mp4': 'video/mp4',
  '.mov': 'video/quicktime',
  '.webm': 'video/webm',
  '.pdf': 'application/pdf',
  '.txt': 'text/plain'
}

const contentType = (path: string) => CONTENT_TYPES[extname(path).toLowerCase()] ?? 'application/octet-stream'

export function ghUploader(endpoint: string, repositoryId: number | undefined): IUploader {
  return {
    upload: async path => {
      const url = new URL(endpoint)
      url.searchParams.set('name', basename(path))
      url.searchParams.set('content_type', contentType(path))
      if (repositoryId !== undefined) url.searchParams.set('repository_id', String(repositoryId))
      const args = ['api', '--method', 'POST', url.toString(), '--input', path, '-H', `Content-Type: ${contentType(path)}`]
      return ghJson(args, z.object({url: z.string().url()}), `upload ${path}`).url
    }
  }
}
