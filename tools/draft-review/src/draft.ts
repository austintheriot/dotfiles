import {createHash} from 'node:crypto'
import {z} from 'zod'
import {Sha256Schema as Sha256} from './ledger'
import type {Parsed} from './parsed'

export type PendingComment = {id: number; body: string}
export type PendingReview = {id: number; body: string; comments: PendingComment[]}

const WrittenCommentSchema = z.object({id: z.number().int().positive(), bodyHash: Sha256}).strict()

export const DraftRecordSchema = z
  .object({
    reviewId: z.number().int().positive(),
    bodyHash: Sha256,
    comments: z.array(WrittenCommentSchema).default([]),
    contributionsDigest: Sha256.optional(),
    head: z.string().min(1),
    url: z.string().min(1)
  })
  .strict()
export type DraftRecord = z.infer<typeof DraftRecordSchema>

const NamedReviewSchema = z.object({reviewId: z.number().int().positive()})

function jsonOf(text: string): unknown {
  try {
    return JSON.parse(text)
  } catch {
    return undefined
  }
}

function consequenceOfDelete(raw: unknown): string {
  const named = NamedReviewSchema.safeParse(raw)
  if (!named.success) return 'After a delete of draft.json, the next create refuses to replace the pending review that draft-review created until you delete that review on GitHub.'
  const id = named.data.reviewId
  return `It names pending review ${id}. After a delete of draft.json, the next create refuses to replace review ${id} until you delete that review on GitHub.`
}

export function parseDraftRecord(text: string): Parsed<DraftRecord> {
  const raw = jsonOf(text)
  const parsed = DraftRecordSchema.safeParse(raw)
  if (parsed.success) return {ok: true, value: parsed.data}
  const detail = raw === undefined ? 'it is not JSON' : parsed.error.issues.map(issue => `${issue.path.join('.') || '(top level)'}: ${issue.message}`).join(', ')
  return {ok: false, reason: `${detail}. Fix it or delete it. ${consequenceOfDelete(raw)}`}
}

export type ReadBack = {kind: 'read'; comments: PendingComment[]} | {kind: 'failed'; reason: string}
export type RecordDecision = {kind: 'record'; record: DraftRecord} | {kind: 'skip'; cause: 'read-failed' | 'mismatch'; reason: string}
export type CreatedDraft = {
  review: {id: number; body: string; url: string}
  head: string
  requestedBodies: readonly string[]
  readBack: ReadBack
  contributionsDigest: string
}

const normalizedBody = (body: string) => body.replace(/\r\n/g, '\n').trimEnd()
const sortedText = (bodies: readonly string[]) => JSON.stringify(bodies.map(normalizedBody).sort())

// A comment the user adds between the create and the read-back would otherwise be recorded as the tool's, and a later replace would delete it.
function readBackMismatch(reviewId: number, requestedBodies: readonly string[], comments: readonly PendingComment[]): string | undefined {
  if (comments.length !== requestedBodies.length) {
    return `GitHub shows ${comments.length} inline comments on pending review ${reviewId}, and the request had ${requestedBodies.length}`
  }
  if (sortedText(comments.map(comment => comment.body)) !== sortedText(requestedBodies)) {
    return `the inline comments on pending review ${reviewId} are not the comments that the request sent`
  }
  return undefined
}

export function draftRecordAfterCreate(created: CreatedDraft): RecordDecision {
  const {review, head, requestedBodies, readBack, contributionsDigest} = created
  if (readBack.kind === 'failed') return {kind: 'skip', cause: 'read-failed', reason: readBack.reason}
  const mismatch = readBackMismatch(review.id, requestedBodies, readBack.comments)
  if (mismatch !== undefined) return {kind: 'skip', cause: 'mismatch', reason: mismatch}
  const comments = readBack.comments.map(comment => ({id: comment.id, bodyHash: bodyHash(comment.body)}))
  return {kind: 'record', record: {reviewId: review.id, bodyHash: bodyHash(review.body), comments, contributionsDigest, head, url: review.url}}
}

export function contributionsDigest(files: readonly {name: string; sha256: string}[]): string {
  const lines = files.map(file => `${file.name}\u0000${file.sha256}`).sort()
  return bodyHash(lines.join('\n'))
}

export type DraftAction =
  | {kind: 'create'}
  | {kind: 'replace'; reviewId: number}
  | {kind: 'refuse'; reviewId: number}
  | {kind: 'submitted'; reviewId: number; state: string}

export type RecordedReview = {kind: 'none'} | {kind: 'found'; record: DraftRecord; state: string} | {kind: 'gone'; record: DraftRecord}

export const bodyHash = (body: string): string => createHash('sha256').update(body, 'utf8').digest('hex')

const commentKeys = (comments: readonly {id: number; bodyHash: string}[]) => comments.map(comment => `${comment.id}:${comment.bodyHash}`).sort()

const commentsUnchanged = (review: PendingReview, lastWritten: DraftRecord) =>
  JSON.stringify(commentKeys(review.comments.map(comment => ({id: comment.id, bodyHash: bodyHash(comment.body)})))) ===
  JSON.stringify(commentKeys(lastWritten.comments))

const isOurs = (review: PendingReview, lastWritten: DraftRecord | undefined) =>
  lastWritten !== undefined &&
  review.id === lastWritten.reviewId &&
  bodyHash(review.body) === lastWritten.bodyHash &&
  commentsUnchanged(review, lastWritten)

// Any state other than PENDING counts as submitted, so a state that GitHub adds later cannot lead to a second post of the same content.
// A submitted review for an earlier head does not block a draft for a new head: the new head is a new round of review.
export function draftAction(existing: readonly PendingReview[], recorded: RecordedReview, head: string): DraftAction {
  if (recorded.kind === 'found' && recorded.state !== 'PENDING' && recorded.record.head === head) {
    return {kind: 'submitted', reviewId: recorded.record.reviewId, state: recorded.state}
  }
  const lastWritten = recorded.kind === 'found' ? recorded.record : undefined
  const [first] = existing
  if (first === undefined) return {kind: 'create'}
  const foreign = existing.find(review => !isOurs(review, lastWritten))
  return foreign === undefined ? {kind: 'replace', reviewId: first.id} : {kind: 'refuse', reviewId: foreign.id}
}
