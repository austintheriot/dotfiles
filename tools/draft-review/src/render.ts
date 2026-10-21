import type {InlineComment, ReviewPayload} from './payload'

const location = (comment: InlineComment) =>
  comment.startLine === undefined || comment.startLine === comment.line ? `line ${comment.line}` : `lines ${comment.startLine}-${comment.line}`

const section = (heading: string, lines: readonly string[]) => (lines.length === 0 ? [] : [`--- ${heading} ---`, lines.join('\n')])

export function renderReview(payload: ReviewPayload): string {
  const parts = [
    payload.body,
    ...payload.comments.flatMap(comment => [`--- inline comment: ${comment.path} ${location(comment)} (${comment.side}) ---`, comment.body]),
    ...section('files to upload', payload.uploads),
    ...section('hand-back (not posted)', payload.handback)
  ]
  return `${parts.join('\n\n')}\n`
}
