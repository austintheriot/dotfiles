import {createHash} from 'node:crypto'
import {z} from 'zod'

export type PendingReview = {id: number; body: string}

export const DraftRecordSchema = z
  .object({reviewId: z.number().int().positive(), bodyHash: z.string().regex(/^[0-9a-f]{64}$/), head: z.string().min(1), url: z.string().min(1)})
  .strict()
export type DraftRecord = z.infer<typeof DraftRecordSchema>

export type DraftAction = {kind: 'create'} | {kind: 'replace'; reviewId: number} | {kind: 'refuse'; reviewId: number}

export const bodyHash = (body: string): string => createHash('sha256').update(body, 'utf8').digest('hex')

const isOurs = (review: PendingReview, lastWritten: DraftRecord | undefined) =>
  lastWritten !== undefined && review.id === lastWritten.reviewId && bodyHash(review.body) === lastWritten.bodyHash

export function draftAction(existing: readonly PendingReview[], lastWritten: DraftRecord | undefined): DraftAction {
  const [first] = existing
  if (first === undefined) return {kind: 'create'}
  const foreign = existing.find(review => !isOurs(review, lastWritten))
  return foreign === undefined ? {kind: 'replace', reviewId: first.id} : {kind: 'refuse', reviewId: foreign.id}
}
