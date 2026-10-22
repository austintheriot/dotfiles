import {mkdtempSync, rmSync, writeFileSync} from 'node:fs'
import {tmpdir} from 'node:os'
import {basename, extname, join} from 'node:path'
import {z} from 'zod'
import type {Repo} from './args'
import {parseRepo} from './args'
import type {PullFile} from './diff-anchors'
import type {DraftRecord, PendingComment, PendingReview, RecordedReview, SubmittedReview} from './draft'
import {DraftReviewError, EXIT} from './exit'
import type {ReviewPayload} from './payload'
import type {IUploader} from './store'

export type GhRun = {code: number; stdout: string; stderr: string}

function gh(args: readonly string[]): GhRun {
  try {
    const run = Bun.spawnSync(['gh', ...args], {stdin: 'ignore', stdout: 'pipe', stderr: 'pipe'})
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

const ReviewSchema = z.object({
  id: z.number().int(),
  state: z.string(),
  body: z.string().nullable(),
  commit_id: z.string().nullable().optional(),
  user: z.object({login: z.string()}).nullable()
})
const ReviewCommentSchema = z.object({id: z.number().int(), body: z.string()})

export function reviewComments(repo: Repo, pr: number, reviewId: number): PendingComment[] {
  const pages = ghJson(
    ['api', `${reviewsPath(repo, pr)}/${reviewId}/comments`, '--paginate', '--slurp'],
    z.array(z.array(ReviewCommentSchema)),
    `list the comments of pending review ${reviewId}`
  )
  return pages.flat().map(comment => ({id: comment.id, body: comment.body}))
}

export type ViewerReviews = {pending: PendingReview[]; submitted: SubmittedReview[]}

export function viewerReviews(repo: Repo, pr: number, login: string): ViewerReviews {
  const pages = ghJson(['api', reviewsPath(repo, pr), '--paginate', '--slurp'], z.array(z.array(ReviewSchema)), `list the reviews of pull request ${pr}`)
  const own = pages.flat().filter(review => review.user?.login === login)
  return {
    pending: own
      .filter(review => review.state === 'PENDING')
      .map(review => ({id: review.id, body: review.body ?? '', comments: reviewComments(repo, pr, review.id)})),
    submitted: own
      .filter(review => review.state !== 'PENDING')
      .map(review => ({id: review.id, state: review.state, commitId: review.commit_id ?? undefined, body: review.body ?? ''}))
  }
}

// A 404 means the review is gone: the user deleted the pending draft, which discards it. Any other failure stops the run.
export function recordedReview(repo: Repo, pr: number, record: DraftRecord | undefined): RecordedReview {
  if (record === undefined) return {kind: 'none'}
  return recordedReviewFrom(gh(['api', '--include', `${reviewsPath(repo, pr)}/${record.reviewId}`]), record)
}

export function recordedReviewFrom(run: GhRun, record: DraftRecord): RecordedReview {
  const reply = parseHttpReply(run.stdout)
  if (reply.status === 404 || (reply.status === undefined && /HTTP 404/.test(run.stderr))) return {kind: 'gone', record}
  const review = parseReply({...run, stdout: reply.body}, z.object({state: z.string()}), `read review ${record.reviewId}, which draft.json names`)
  return {kind: 'found', record, state: review.state}
}

export const pullRequestHead = (repo: Repo, pr: number): string =>
  ghJson(['api', `repos/${repo.owner}/${repo.name}/pulls/${pr}`], z.object({head: z.object({sha: z.string()})}), `read the head of pull request ${pr}`).head.sha

const PullFileSchema =z.object({filename: z.string(), patch: z.string().optional()})

export function pullRequestFiles(repo: Repo, pr: number): PullFile[] {
  const pages = ghJson(
    ['api', `repos/${repo.owner}/${repo.name}/pulls/${pr}/files`, '--paginate', '--slurp'],
    z.array(z.array(PullFileSchema)),
    `list the changed files of pull request ${pr}`
  )
  return pages.flat().map(file => ({filename: file.filename, patch: file.patch}))
}

export function deleteReview(repo: Repo, pr: number, reviewId: number): void {
  const run = gh(['api', '--method', 'DELETE', `${reviewsPath(repo, pr)}/${reviewId}`])
  if (run.code !== 0) throw new DraftReviewError(`gh could not delete pending review ${reviewId}: ${run.stderr.trim()}`, EXIT.unavailable)
}

export type CreatedReview = {id: number; url: string; body: string}
export type CreateOutcome = {kind: 'created'; review: CreatedReview} | {kind: 'pending-exists'}
export type HttpReply = {status: number | undefined; body: string}

export function parseHttpReply(text: string): HttpReply {
  const status = /^HTTP\/[\d.]+ (\d{3})/.exec(text)?.[1]
  if (status === undefined) return {status: undefined, body: text}
  const separator = /\r?\n\r?\n/.exec(text)
  return {status: Number(status), body: separator === null ? '' : text.slice(separator.index + separator[0].length)}
}

export const isPendingReviewConflict = (reply: HttpReply): boolean => reply.status === 422 && /one pending review/i.test(reply.body)

export function reviewRequest(head: string, payload: ReviewPayload) {
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
// The request file must come from reviewRequest, which never sets `event`.
export function createPendingReview(repo: Repo, pr: number, requestPath: string): CreateOutcome {
  const run = gh(['api', '--include', '--method', 'POST', reviewsPath(repo, pr), '--input', requestPath])
  const reply = parseHttpReply(run.stdout)
  if (isPendingReviewConflict(reply)) return {kind: 'pending-exists'}
  const created = parseReply({...run, stdout: reply.body}, z.object({id: z.number().int(), html_url: z.string(), body: z.string().nullable()}), 'create the pending review')
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
    upload: async ({path, bytes}) => {
      const url = new URL(endpoint)
      url.searchParams.set('name', basename(path))
      url.searchParams.set('content_type', contentType(path))
      if (repositoryId !== undefined) url.searchParams.set('repository_id', String(repositoryId))
      // gh streams stdin as a chunked body, and the upload endpoint refuses a body without a Content-Length.
      const dir = mkdtempSync(join(tmpdir(), 'draft-review-upload-'))
      try {
        const body = join(dir, 'body')
        writeFileSync(body, bytes, {mode: 0o600})
        const args = ['api', '--method', 'POST', url.toString(), '--input', body, '-H', `Content-Type: ${contentType(path)}`]
        return parseReply(gh(args), z.object({url: z.string().url()}), `upload ${path}`).url
      } finally {
        rmSync(dir, {recursive: true, force: true})
      }
    }
  }
}
