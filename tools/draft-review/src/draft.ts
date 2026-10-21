import {createHash} from 'node:crypto'
import {z} from 'zod'
import {Sha256Schema as Sha256} from './ledger'

export type PendingComment = {id: number; body: string}
export type PendingReview = {id: number; body: string; comments: PendingComment[]}

const WrittenCommentSchema = z.object({id: z.number().int().positive(), bodyHash: Sha256}).strict()

export const DraftRecordSchema = z
  .object({
    reviewId: z.number().int().positive(),
    bodyHash: Sha256,
    comments: z.array(WrittenCommentSchema).default([]),
    head: z.string().min(1),
    url: z.string().min(1)
  })
  .strict()
export type DraftRecord = z.infer<typeof DraftRecordSchema>

export type DraftAction = {kind: 'create'} | {kind: 'replace'; reviewId: number} | {kind: 'refuse'; reviewId: number}

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

export function draftAction(existing: readonly PendingReview[], lastWritten: DraftRecord | undefined): DraftAction {
  const [first] = existing
  if (first === undefined) return {kind: 'create'}
  const foreign = existing.find(review => !isOurs(review, lastWritten))
  return foreign === undefined ? {kind: 'replace', reviewId: first.id} : {kind: 'refuse', reviewId: foreign.id}
}
