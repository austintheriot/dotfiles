import type {ReviewPayload} from './payload'
import {lineLabel} from './payload'
import type {StrayPath} from './uploads'

const section = (heading: string, lines: readonly string[]) => (lines.length === 0 ? [] : [`--- ${heading} ---`, lines.join('\n')])

export function renderStrayPaths(strays: readonly StrayPath[]): string {
  const parts = section('local paths that create refuses: list each file in uploads, or remove the path', strays.map(stray => `${stray.location}: ${stray.path}`))
  return parts.length === 0 ? '' : `${parts.join('\n\n')}\n`
}

export function renderReview(payload: ReviewPayload): string {
  const parts = [
    payload.body,
    ...payload.comments.flatMap(comment => [`--- inline comment: ${comment.path} ${lineLabel(comment)} (${comment.side}) ---`, comment.body]),
    ...section('files to upload', payload.uploads),
    ...section('hand-back (not posted)', payload.handback)
  ]
  return `${parts.join('\n\n')}\n`
}
